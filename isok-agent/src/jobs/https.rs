use std::{
    io::{self, Write},
    net::TcpStream,
    sync::{Arc, OnceLock},
    time::Instant,
};

use hickory_resolver::{error::ResolveError, TokioAsyncResolver};
use prost_types::Timestamp;
use rustls::{
    pki_types::{DnsName, ServerName},
    ClientConfig, ClientConnection,
};
use rustls_native_certs::CertificateResult;
use serde::{Deserialize, Serialize};
use tracing::{info, trace, warn};
use x509_parser::{nom::Finish, prelude::X509Certificate, time::ASN1Time};

use isok_data::broker_rpc::{check_result::Details, CheckJobStatus, JobDetailsHttps};

use crate::batch_sender::JobResult;

use super::{Execute, JobError};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Hash)]
pub struct HttpsJob {
    #[serde(with = "dns_name")]
    host: DnsName<'static>,
    #[serde(default = "default_port")]
    port: u16,
}

const fn default_port() -> u16 {
    443
}

impl HttpsJob {
    pub const fn new(host: DnsName<'static>) -> Self {
        Self {
            host,
            port: default_port(),
        }
    }

    pub const fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }
}

mod dns_name {
    use rustls::pki_types::DnsName;
    use serde::{de, Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &DnsName, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(value.as_ref())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<DnsName<'static>, D::Error> {
        let s = String::deserialize(deserializer)?;
        DnsName::try_from(s).map_err(de::Error::custom)
    }
}

// DNS RESOLVER

#[derive(Debug, thiserror::Error)]
#[error("failed to create resolver from system configuration: {0}")]
pub struct DnsResolverError(ResolveError);

/// Returns a DNS `Resolver` based on the system configuration.
fn dns_resolver() -> Result<Arc<TokioAsyncResolver>, DnsResolverError> {
    static RESOLVER: OnceLock<Arc<TokioAsyncResolver>> = OnceLock::new();

    match RESOLVER.get() {
        Some(resolver) => Ok(resolver.clone()),
        None => {
            let resolver =
                TokioAsyncResolver::tokio_from_system_conf().map_err(DnsResolverError)?;
            let resolver = Arc::new(resolver);

            match RESOLVER.set(resolver.clone()) {
                Ok(()) => Ok(resolver),
                // TOCTOU: use resolver set by another thread
                Err(resolver) => Ok(resolver),
            }
        }
    }
}

// ROOT CERTS

#[derive(Debug, thiserror::Error)]
pub enum RootCertsError {
    #[error("failed to load native certificates: {0:?}")]
    Load(Vec<rustls_native_certs::Error>),
    #[error("no native certificates found")]
    Empty,
}

// Loads the root certificates from the platform’s native certificate store.
fn load_root_certificates() -> Result<rustls::RootCertStore, RootCertsError> {
    let CertificateResult { certs, errors, .. } = rustls_native_certs::load_native_certs();
    if !errors.is_empty() {
        return Err(RootCertsError::Load(errors));
    }
    let mut root_cert_store = rustls::RootCertStore::empty();
    let (n, ignored) = root_cert_store.add_parsable_certificates(certs);
    if ignored != 0 {
        trace!("ignored {ignored} ancient or syntactically invalid native certificates");
    }
    if root_cert_store.is_empty() {
        return Err(RootCertsError::Empty);
    }
    trace!("loaded {n} native certificates");
    Ok(root_cert_store)
}

/// Returns a `ClientConfig` based on the native root certificates.
pub fn client_config() -> Result<Arc<ClientConfig>, RootCertsError> {
    static CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();

    match CONFIG.get() {
        Some(config) => Ok(config.clone()),
        None => {
            match load_root_certificates() {
                Err(error) => Err(error),
                Ok(root_cert_store) => {
                    let config = ClientConfig::builder()
                        .with_root_certificates(Arc::new(root_cert_store))
                        .with_no_client_auth();
                    let config = Arc::new(config);
                    match CONFIG.set(config.clone()) {
                        // TOCTOU: use config set by another thread
                        Err(config) => Ok(config),
                        Ok(()) => Ok(config),
                    }
                }
            }
        }
    }
}

// TLS HANDSHAKE

#[derive(Debug, thiserror::Error)]
pub enum HandshakeError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("failed to process new packets: {0}")]
    ProcessNewPackets(#[from] rustls::Error),
}

/// Performs the TLS handshake with the server.
fn handshake(
    client_connection: &mut ClientConnection,
    stream: &mut TcpStream,
) -> Result<(), HandshakeError> {
    let mut backoff = 1;
    while client_connection.is_handshaking() {
        while client_connection.wants_write() {
            client_connection.write_tls(stream)?;
            stream.flush()?;
        }
        match client_connection.read_tls(stream) {
            Err(error) => match error.kind() {
                io::ErrorKind::WouldBlock | io::ErrorKind::Other if backoff < 64 => backoff *= 2,
                _ => return Err(HandshakeError::Io(error)),
            },
            Ok(0) => break,
            Ok(_) => {
                let _io_state = client_connection.process_new_packets()?;
            }
        }
    }
    Ok(())
}

// READ PEER CERTIFICATE

#[derive(Debug, thiserror::Error)]
pub enum CertError {
    #[error("peer certificates are unavailable")]
    Unavailable,
    #[error("no peer certificates found")]
    Empty,
    #[error("failed to parse x509 certificate: {0}")]
    Invalid(x509_parser::error::X509Error),
}

/// Read the last x509 certificate from `client_connection`.
fn read_certificate(
    client_connection: &ClientConnection,
) -> Result<X509Certificate<'_>, CertError> {
    let certificate = client_connection
        .peer_certificates()
        .ok_or(CertError::Unavailable)? // we could `expect` here instead as it denotes handshake failure
        .first()
        .ok_or(CertError::Empty)?;

    let (rem, certificate) = x509_parser::parse_x509_certificate(certificate)
        .finish()
        .map_err(CertError::Invalid)?;

    if !rem.is_empty() {
        trace!(
            "unparsed trailing bytes of certificate: {:?}",
            String::from_utf8_lossy(rem)
        );
    }

    Ok(certificate)
}

// TCP CONNECTION

#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error("failed to resolve IP address: {0}")]
    IpLookup(ResolveError),
    #[error("no IP address found")]
    NotFound,
    #[error("failed to start TCP connection: {0}")]
    Io(io::Error),
}

/// Lookup IP of `host` using the given DNS resolver then start a TCP connection using the resolved IP adress and `port`.
async fn connect(
    dns_resolver: &TokioAsyncResolver,
    host: &str,
    port: u16,
) -> Result<(TcpStream, std::time::Duration), ConnectError> {
    match dns_resolver.lookup_ip(host).await {
        Err(error) => Err(ConnectError::IpLookup(error)),
        Ok(ips) => match ips.iter().next() {
            None => Err(ConnectError::NotFound),
            Some(ip_addr) => {
                let now = Instant::now();
                match TcpStream::connect((ip_addr, port)) {
                    Err(error) => Err(ConnectError::Io(error)),
                    Ok(stream) => {
                        let elapsed = now.elapsed();
                        Ok((stream, elapsed))
                    }
                }
            }
        },
    }
}

impl Execute for HttpsJob {
    async fn execute(&self, job_result: &mut JobResult) -> Result<(), JobError> {
        info!("Executing HTTPS job for '{}'", self.host.as_ref());

        // run these first to ensure job fails if the environment is not correctly configured
        let config = client_config().map_err(JobError::RootCertsError)?;
        let dns_resolver = dns_resolver().map_err(JobError::DnsConfigError)?;

        // resolve IP address of host and start TCP connection
        let mut stream = match connect(&dns_resolver, self.host.as_ref(), self.port).await {
            Err(error) => {
                warn!("failed to connect to host: {error:?}");
                job_result.set_status(CheckJobStatus::Unreachable);
                job_result.set_error(error);
                return Ok(());
            }
            Ok((stream, latency)) => {
                job_result.set_status(CheckJobStatus::Reachable);
                job_result.set_latency(latency);
                stream
            }
        };

        // create TLS connection on top of the TCP stream
        let client_connection =
            match ClientConnection::new(config, ServerName::DnsName(self.host.clone())) {
                Err(error) => {
                    warn!("failed to create TLS connection: {error}");
                    job_result.set_error(error);
                    return Ok(());
                }
                Ok(mut client_connection) => match handshake(&mut client_connection, &mut stream) {
                    Err(error) => {
                        warn!("failed to perform TLS handshake: {error}");
                        job_result.set_error(error);
                        return Ok(());
                    }
                    Ok(()) => client_connection,
                },
            };

        match read_certificate(&client_connection) {
            Err(error) => {
                warn!("failed to read x509 certificate: {error}");
                job_result.set_error(error);
                return Ok(());
            }
            Ok(certificate) => {
                let validity = certificate.validity();
                job_result.set_details(Details::DetailsHttps(JobDetailsHttps {
                    expired_after: Some(timestamp(validity.not_after)),
                    not_active_before: Some(timestamp(validity.not_before)),
                    is_valid: validity.is_valid(),
                }));
            }
        }

        Ok(())
    }
}

fn timestamp(time: ASN1Time) -> Timestamp {
    Timestamp {
        seconds: time.timestamp(),
        nanos: 0,
    }
}

#[cfg(test)]
mod tests {

    use pretty_assertions::assert_eq;

    use isok_data::JobId;

    use super::*;

    #[tokio::test]
    async fn test_https_job_ok() {
        tracing_subscriber::fmt::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            // .with_max_level(tracing::level_filters::LevelFilter::TRACE)
            .init();

        let _ = rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .unwrap();

        for host in ["google.com", "clever-cloud.com"] {
            let job = HttpsJob {
                host: DnsName::try_from(host).unwrap(),
                port: 443,
            };
            let mut job_result = JobResult::new(JobId::generate(), format!("test {host:?}"));
            let result = job.execute(&mut job_result).await;
            assert!(result.is_ok()); // NOTE: might fail in CI environment
            assert_eq!(job_result.status, CheckJobStatus::Reachable);
            assert!(job_result.details.is_some_and(|details| match details {
                Details::DetailsHttps(details) => details.is_valid,
                _ => false,
            }));
        }
    }

    #[tokio::test]
    async fn test_https_job_ko() {
        tracing_subscriber::fmt::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            // .with_max_level(tracing::level_filters::LevelFilter::TRACE)
            .init();

        let _ = rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .unwrap();

        let job = HttpsJob {
            host: DnsName::try_from("nonexistentdnsnameatleastuntilnow.com").unwrap(),
            port: 443,
        };
        let mut job_result =
            JobResult::new(JobId::generate(), "test non-existent domain".to_string());
        let result = job.execute(&mut job_result).await;
        assert!(result.is_ok()); // NOTE: might fail in CI environment
        assert_eq!(job_result.status, CheckJobStatus::Unreachable);
        assert!(job_result.error.is_some());
    }
}
