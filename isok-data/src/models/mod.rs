mod agent;
mod checks;
mod organisation;
mod region;
mod user;

pub use agent::*;
pub use checks::*;
pub use organisation::*;
pub use region::*;
pub use user::*;

pub use refined::RefinementOps;
use refined::boundable::unsigned::ClosedInterval;
use refined::string::Regex;
use refined::{Refinement, TypeString, type_string};
use schemars::schema::{
    InstanceType, NumberValidation, Schema, SchemaObject, SingleOrVec, StringValidation,
};
use schemars::{JsonSchema, SchemaGenerator};
use std::collections::HashMap;

pub type Tags = HashMap<String, Option<String>>;

type_string!(
    EmailRegex,
    r#"(?:[a-z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[a-z0-9!#$%&'*+/=?^_`{|}~-]+)*|"(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21\x23-\x5b\x5d-\x7f]|\\[\x01-\x09\x0b\x0c\x0e-\x7f])*")@(?:(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z0-9](?:[a-z0-9-]*[a-z0-9])?|\[(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?|[a-z0-9-]*[a-z0-9]:(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21-\x5a\x53-\x7f]|\\[\x01-\x09\x0b\x0c\x0e-\x7f])+)\])"#
);
pub type Email = Refinement<String, Regex<EmailRegex>>;

impl JsonSchema for EmailRegex {
    fn schema_name() -> String {
        "Email".to_string()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(SingleOrVec::from(InstanceType::String)),
            format: Some("email".to_string()),
            string: Some(Box::new(StringValidation {
                min_length: Some(3),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

pub type Password = Refinement<String, ClosedInterval<6, 128>>;

struct PasswordSchema;

impl JsonSchema for PasswordSchema {
    fn schema_name() -> String {
        "Password".to_string()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(SingleOrVec::from(InstanceType::String)),
            string: Some(Box::new(StringValidation {
                min_length: Some(6),
                max_length: Some(128),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

pub type OrgName = Refinement<String, ClosedInterval<1, 64>>;

pub type CheckName = OrgName;

struct NameSchema<const NAME: u8>;

impl<const NAME: u8> JsonSchema for NameSchema<NAME> {
    fn schema_name() -> String {
        match NAME {
            0 => "OrgName",
            1 => "CheckName",
            _ => "Name",
        }
        .to_string()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(SingleOrVec::from(InstanceType::String)),
            string: Some(Box::new(StringValidation {
                min_length: Some(1),
                max_length: Some(64),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

pub type U32InRange<const MIN: usize, const MAX: usize> = Refinement<u32, ClosedInterval<MIN, MAX>>;

mod duration_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    #[inline]
    pub fn serialize<S: Serializer>(duration: &Duration, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_u64(duration.as_secs())
    }

    #[inline]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(s))
    }
}

struct DurationSchema<const MIN: u64, const MAX: u64>;

impl<const MIN: u64, const MAX: u64> JsonSchema for DurationSchema<MIN, MAX> {
    fn schema_name() -> String {
        "Duration".to_string()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(SingleOrVec::from(InstanceType::Number)),
            number: Some(Box::new(NumberValidation {
                maximum: Some(MAX as f64),
                minimum: Some(MIN as f64),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}
