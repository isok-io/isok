pub mod proxy {
    pub use std::sync::Arc;
    pub use std::time::Duration;

    pub use log::{error, trace};
    pub use reqwest::header::HeaderMap;
    pub use reqwest::ClientBuilder;
    pub use serde::de::DeserializeOwned;

    pub use crate::api::ApiHandler;

    pub async fn apis_req_vec<T>(state: Arc<ApiHandler>, path: &str) -> Vec<T>
    where
        T: DeserializeOwned + Clone,
    {
        let client = ClientBuilder::new()
            .default_headers(HeaderMap::new())
            .build()
            .unwrap();
        let mut res: Vec<Vec<T>> = Vec::new();

        for uri in state.apis.values() {
            trace!("Proxying to {uri}...");
            let r = match client
                .get(format!("{uri}{path}"))
                .timeout(Duration::from_secs(15))
                .send()
                .await
            {
                Ok(res) => res.json::<Vec<T>>().await.unwrap_or_else(|e| {
                    error!("Error while parsing response of api {uri}: {e}");
                    Vec::<T>::with_capacity(0)
                }),
                Err(e) => {
                    error!("Error while contacting api {uri}: {e}");
                    Vec::<T>::with_capacity(0)
                }
            };

            res.push(r);
        }

        res.iter().flatten().map(|e| e.to_owned()).collect()
    }

    pub async fn apis_req<T>(state: Arc<ApiHandler>, path: &str, id: String) -> Option<T>
    where
        T: DeserializeOwned + Clone,
    {
        let client = ClientBuilder::new()
            .default_headers(HeaderMap::new())
            .build()
            .unwrap();

        for uri in state.apis.values() {
            let r = match client
                .get(format!("{uri}{path}/{id}"))
                .timeout(Duration::from_secs(15))
                .send()
                .await
            {
                Ok(res) => res.json::<T>().await.map(|t| Some(t)).unwrap_or_else(|e| {
                    error!("Error while parsing response of api {uri}: {e}");
                    None
                }),
                Err(e) => {
                    error!("Error while contacting api {uri}: {e}");
                    None
                }
            };

            if let Some(output) = r {
                return Some(output);
            }
        }

        None
    }
}

pub mod auth {
    pub mod password {
        pub use argon2::password_hash::rand_core::{CryptoRngCore, OsRng};
        pub use argon2::password_hash::{PasswordHashString, SaltString};
        pub use argon2::{Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version};

        pub use crate::api::errors::PasswordHashError;

        pub fn hash_password(
            password: &str,
            argon2_params: &Params,
        ) -> Result<PasswordHashString, PasswordHashError> {
            let argon2 = Argon2::new(
                Algorithm::default(),
                Version::default(),
                argon2_params.clone(),
            );
            let salt = generate_salt(&mut OsRng);
            Ok(argon2
                .hash_password(password.as_bytes(), &salt)
                .map_err(PasswordHashError)?
                .serialize())
        }

        pub fn verify_password(
            password: &str,
            password_hash: &PasswordHashString,
            argon2_params: &Params,
        ) -> bool {
            let argon2 = Argon2::new(
                Algorithm::default(),
                Version::default(),
                argon2_params.clone(),
            );

            argon2
                .verify_password(password.as_bytes(), &password_hash.password_hash())
                .map(|_| true)
                .unwrap_or(false)
        }

        fn generate_salt(mut rng: impl CryptoRngCore) -> SaltString {
            let mut bytes = [0u8; 32];
            rng.fill_bytes(&mut bytes);
            SaltString::encode_b64(&bytes).expect("salt string invariant violated")
        }
    }

    pub mod token {
        pub use std::str::FromStr;

        pub use crate::api::errors::BiscuitError;
        pub use biscuit_auth::macros::biscuit;
        pub use biscuit_auth::{Biscuit, KeyPair, PrivateKey, UnverifiedBiscuit};
        pub use uuid::Uuid;

        pub fn build_token(uuid: Uuid, private_key: &PrivateKey) -> Result<String, BiscuitError> {
            let uuid = uuid.as_hyphenated().to_string();
            let keypair = KeyPair::from(private_key);
            let builder = biscuit!("user({uuid});");
            let biscuit: Biscuit = builder.build(&keypair).map_err(BiscuitError::from_token)?;
            biscuit.to_base64().map_err(BiscuitError::from_token)
        }

        pub fn get_user_id_from_token(
            token: String,
            private_key: &PrivateKey,
        ) -> Result<Uuid, BiscuitError> {
            let biscuit =
                UnverifiedBiscuit::from_base64(token).map_err(BiscuitError::from_token)?;
            let biscuit = biscuit
                .check_signature(|_| KeyPair::from(private_key).public())
                .map_err(BiscuitError::from_format)?;
            let res: Vec<(String,)> = biscuit
                .authorizer()
                .map_err(BiscuitError::from_token)?
                .query("data($uuid) <- user($uuid)")
                .map_err(BiscuitError::from_token)?;
            Uuid::from_str(&res.first().ok_or(BiscuitError::none())?.0)
                .map_err(BiscuitError::from_uuid)
        }
    }
}

pub mod validator {
    pub use regex::Regex;

    pub use ping_data::owner::UserInput;

    pub use crate::api::errors::InvalidInput;

    pub fn valid_email<'a>(email: String) -> Result<(), InvalidInput<&'a str, &'a str>> {
        let email_regex = Regex::new(
            r#"(?:[a-z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[a-z0-9!#$%&'*+/=?^_`{|}~-]+)*|"(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21\x23-\x5b\x5d-\x7f]|\\[\x01-\x09\x0b\x0c\x0e-\x7f])*")@(?:(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z0-9](?:[a-z0-9-]*[a-z0-9])?|\[(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?|[a-z0-9-]*[a-z0-9]:(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21-\x5a\x53-\x7f]|\\[\x01-\x09\x0b\x0c\x0e-\x7f])+)\])"#,
        )
            .unwrap();

        if email_regex.is_match(&email) {
            Ok(())
        } else {
            Err(InvalidInput::new("Invalid email address"))
        }
    }

    pub fn valid_password<'a>(password: String) -> Result<(), InvalidInput<&'a str, &'a str>> {
        if (8..=128).contains(&password.len()) {
            Ok(())
        } else {
            Err(InvalidInput::new("Invalid password"))
        }
    }

    pub fn valid_string<'a>(string: String) -> Result<(), InvalidInput<&'a str, &'a str>> {
        if !string.is_empty() && !string.starts_with(' ') {
            Ok(())
        } else {
            Err(InvalidInput::new("Invalid field"))
        }
    }

    pub fn valid_username<'a>(username: String) -> Result<(), InvalidInput<&'a str, &'a str>> {
        valid_string(username.clone())
            .map_err(|_| InvalidInput::new("Invalid username"))
            .and(if username.is_ascii() {
                Ok(())
            } else {
                Err(InvalidInput::new(
                    "Username should contains only ascii chars",
                ))
            })
            .and(if username.len() >= 3 {
                Ok(())
            } else {
                Err(InvalidInput::new("Username should be longer"))
            })
            .and(if username.len() <= 32 {
                Ok(())
            } else {
                Err(InvalidInput::new("Username should be shorter"))
            })
            .and(if !username.contains(' ') {
                Ok(())
            } else {
                Err(InvalidInput::new("Username should not contains space"))
            })
    }

    pub fn valid_user_input<'a>(user: UserInput) -> Result<(), InvalidInput<&'a str, &'a str>> {
        valid_username(user.username)
            .map_err(|e| e.with_field_name("username"))
            .and(valid_password(user.password).map_err(|e| e.with_field_name("password")))
            .and(valid_email(user.email_address).map_err(|e| e.with_field_name("email_address")))
    }
}