use crate::config::AgentsHandlerConfig;
use crate::db::DbHandler;
use crate::errors::Result;
use isok_data::models::{Agent, AgentInput, ApiCheck, Check};
use sqlx::types::chrono::Utc;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use tokio::pin;
use tokio::sync::RwLock;
use tokio::sync::watch::Receiver;
use tokio::task::JoinSet;
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

pub struct AgentsHandler {
    config: AgentsHandlerConfig,
    db: DbHandler,
    agents_tries: RwLock<HashMap<String, u8>>,
    client: reqwest::Client,
}

impl AgentsHandler {
    pub fn new(
        config: AgentsHandlerConfig,
        db: DbHandler,
        js: &mut JoinSet<Result<()>>,
        shutdown_rx: Receiver<()>,
    ) -> Arc<AgentsHandler> {
        let ah = Arc::new(AgentsHandler {
            config,
            db,
            agents_tries: Default::default(),
            client: Default::default(),
        });

        js.spawn(ah.clone().worker(shutdown_rx));

        ah
    }

    async fn worker(self: Arc<Self>, mut shutdown_rx: Receiver<()>) -> Result<()> {
        info!("Trying to send checks to agents");
        let checks = self.db.agents_get_incomplete_checks().await?;
        let checks = self.db.checks_get_by_ids(checks).await?;
        let checks_len = checks.len();
        if let Err(error) = self.add_checks(checks).await {
            error!(?error, "Error while adding {checks_len} checks");
        }

        let itv = tokio::time::interval(self.config.healthcheck_itv);
        pin!(itv);

        loop {
            tokio::select! {
                _ = itv.tick() => {
                    debug!("Ticked");

                    let agents = self.db.agents_get_all(Some(Utc::now() - self.config.healthcheck_itv + Duration::from_secs(1))).await?;

                    for agent in agents {
                        let zone =agent.zone.as_hyphenated().to_string();

                        match self.agent_healtcheck(&agent).await {
                            Ok(_) => {
                                self.db.agents_update_healthchecked_at(&agent.id, Utc::now()).await?;
                                self.agents_tries.write().await.remove(&agent.id);
                            }
                            Err(error) => {
                                warn!(agent=agent.id, zone, ?error, "Failed to reach the agent");

                                let mut agent_tries = self.agents_tries.write().await;
                                let tries = agent_tries.get(&agent.id).unwrap_or(&0) + 1;
                                if tries >= self.config.max_retries {
                                    error!(agent=agent.id, zone, "Agent is unreachable, deregistering it");

                                    let s = self.clone();
                                    tokio::spawn(async move {
                                        if let Err(error) = s.remove_agent(agent.id.clone()).await {
                                            error!(agent=agent.id, zone, ?error, "Error while deregistering the agent")
                                        }
                                    });
                                } else {
                                    agent_tries.insert(agent.id.clone(), tries);
                                }
                            }
                        }
                    }

                }
                _ = shutdown_rx.changed() => {
                    info!("Shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    async fn agent_healtcheck(self: &Arc<Self>, agent: &Agent) -> Result<()> {
        debug!(
            agent = agent.id,
            zone = agent.zone.as_hyphenated().to_string(),
            "Healthchecking agent"
        );

        self.client
            .get(format!("{}/ping", agent.endpoint))
            .send()
            .await?;

        Ok(())
    }

    pub async fn add_agent(self: &Arc<Self>, agent: AgentInput) -> Result<()> {
        let db_agent = self.db.agents_get_by_id(&agent.id).await?;
        if let Some(db_agent) = db_agent {
            warn!("Agent {} already registered, deleting it", db_agent.id);
            self.db.agents_delete(db_agent.id).await?;
        }

        let agent = agent.into();

        self.db.agents_insert(&agent).await?;

        let agent_id = agent.id.clone();
        let s = self.clone();
        tokio::spawn(async move {
            let mut itv = interval(Duration::from_secs(5)); // Should be replaced with healthcheck
            itv.tick().await;
            itv.tick().await;

            let Ok(checks) = s.db.agents_get_incomplete_checks_by_zone(agent.zone).await else {
                error!("Failed to get incomplete checks for zone {}", agent.zone);
                return;
            };
            if !checks.is_empty() {
                let Ok(checks) = s.db.checks_get_by_ids(checks).await else {
                    error!("Failed to get checks");
                    return;
                };
                let agent_id = agent.id.clone();
                let checks_len = checks.len();

                if let Err(error) = s.add_checks(checks).await {
                    error!(
                        ?error,
                        "Error while adding {checks_len} checks after agent {agent_id} joined"
                    );
                }
            }
        });

        info!(
            agent = agent_id,
            zone = agent.zone.as_hyphenated().to_string(),
            "Registered"
        );

        Ok(())
    }

    pub async fn remove_agent(self: &Arc<Self>, agent_id: String) -> Result<()> {
        let checks = self.db.agents_delete(agent_id).await?;
        let checks = self.db.checks_get_by_ids(checks).await?;
        self.add_checks(checks).await
    }

    async fn choose_agents<'a>(
        self: &Arc<Self>,
        agents: Vec<&'a Agent>,
        zac: &mut HashMap<Uuid, HashSet<AgentChecksVec<'a>>>,
        api_check: &ApiCheck,
    ) -> Result<Vec<&'a String>> {
        let mut zones = HashSet::new();

        for zone in &api_check.zones {
            zones.extend(
                self.db
                    .zones_resolve_check_zone(zone)
                    .await?
                    .into_iter()
                    .map(|z| z.id),
            );
        }

        let agent_tries = self.agents_tries.read().await;

        let agents = agents
            .iter()
            .filter(|agent| !agent_tries.contains_key(&agent.id) && zones.contains(&agent.zone));

        for agent in agents {
            let checks = self.db.agents_get_checks(&agent.id).await?;
            let ac = AgentChecksVec { agent, checks };

            if let Entry::Vacant(a) = zac.entry(agent.zone) {
                a.insert(HashSet::from([ac]));
            } else {
                zac.get_mut(&agent.zone)
                    .map(|a: &mut HashSet<AgentChecksVec>| a.insert(ac));
            }
        }

        let mut res = Vec::with_capacity(zones.len());

        for (z, ac) in zac {
            if !ac.iter().all(|s| !s.checks.contains(&api_check.inner.id)) {
                warn!(
                    "Check {} already registered for zone {z}",
                    api_check.inner.id
                );
                continue;
            }
            if let Some(mut agent) = ac.iter().min_by_key(|s| s.len()).cloned() {
                res.push(&agent.agent.id);
                agent.checks.push(api_check.inner.id);
                ac.replace(agent);
            } else {
                warn!("No agents is alive in zone {z}");
            }
        }

        Ok(res)
    }

    pub async fn add_checks(self: &Arc<Self>, api_checks: Vec<ApiCheck>) -> Result<()> {
        let agents = self
            .db
            .agents_get_all(None)
            .await?
            .into_iter()
            .map(|a| (a.id.clone(), a))
            .collect::<HashMap<_, _>>();
        let mut zac = HashMap::new();
        let mut agents_checks: HashMap<String, Vec<Check>> = HashMap::new();

        for check in api_checks {
            let chosen_agents = self
                .choose_agents(agents.values().collect(), &mut zac, &check)
                .await?;

            for agent in chosen_agents {
                if let Some(checks) = agents_checks.get_mut(agent) {
                    checks.push(check.inner.clone());
                } else {
                    agents_checks.insert(agent.clone(), vec![check.inner.clone()]);
                }
            }
        }

        for (agent, checks) in agents_checks {
            let Agent {
                zone,
                endpoint,
                token,
                ..
            } = agents.get(&agent).unwrap();

            let zone = zone.as_hyphenated().to_string();

            let tx = match self
                .db
                .agents_add_checks(&agent, checks.iter().map(|c| c.id).collect())
                .await
            {
                Ok(tx) => tx,
                Err(error) => {
                    error!(
                        agent,
                        zone,
                        ?error,
                        "Error while inserting checks in the database"
                    );
                    continue;
                }
            };

            match self
                .client
                .post(format!("{endpoint}/checks"))
                .header("Authorization", format!("Bearer {token}"))
                .json(&checks)
                .send()
                .await
            {
                Ok(res) if res.status().is_success() => {
                    tx.commit().await?;
                    info!(agent, zone, "{} checks added", checks.len());
                }
                Ok(res) => {
                    let status = res.status().as_u16();
                    let error = res.text().await?;
                    error!(agent = agent, zone, error, "Agent returned error {status}");
                }
                Err(error) => {
                    error!(
                        agent,
                        zone,
                        ?error,
                        "Error while sending checks to the agent"
                    );
                    continue;
                }
            }
        }

        Ok(())
    }

    pub async fn add_check(self: &Arc<Self>, api_check: ApiCheck) -> Result<()> {
        self.add_checks(vec![api_check]).await
    }

    pub async fn remove_checks(self: &Arc<Self>, checks: Vec<Uuid>) -> Result<()> {
        let mut agents = HashMap::new();

        for check in checks {
            for agent in self.db.agents_get_by_check(check).await? {
                agents
                    .entry(agent)
                    .and_modify(|checks: &mut Vec<Uuid>| checks.push(check))
                    .or_insert(vec![check]);
            }
        }

        for (
            Agent {
                endpoint,
                token,
                id,
                zone,
                ..
            },
            checks,
        ) in agents
        {
            let zone = zone.as_hyphenated().to_string();

            if let Err(error) = self
                .client
                .delete(format!("{endpoint}/checks"))
                .header("Authorization", format!("Bearer {token}"))
                .json(&checks)
                .send()
                .await
            {
                error!(agent = id, zone, ?error, "Errors while removing checks");
            } else {
                self.db.agents_delete_checks(&id, &checks).await?;
                info!(agent = id, zone, "Removed {} checks", checks.len());
            }
        }

        Ok(())
    }

    pub async fn remove_check(self: &Arc<Self>, check: Uuid) -> Result<()> {
        self.remove_checks(vec![check]).await
    }
}

#[derive(Clone)]
struct AgentChecksVec<'a> {
    agent: &'a Agent,
    checks: Vec<Uuid>,
}

impl Deref for AgentChecksVec<'_> {
    type Target = Vec<Uuid>;

    fn deref(&self) -> &Self::Target {
        &self.checks
    }
}

impl Hash for AgentChecksVec<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.agent.id.as_bytes())
    }
}

impl Eq for AgentChecksVec<'_> {}

impl PartialEq<Self> for AgentChecksVec<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.agent.id == other.agent.id
    }
}
