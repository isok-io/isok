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
use std::collections::HashMap;

pub type Tags = HashMap<String, Option<String>>;

type_string!(
    EmailRegex,
    r#"(?:[a-z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[a-z0-9!#$%&'*+/=?^_`{|}~-]+)*|"(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21\x23-\x5b\x5d-\x7f]|\\[\x01-\x09\x0b\x0c\x0e-\x7f])*")@(?:(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z0-9](?:[a-z0-9-]*[a-z0-9])?|\[(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?|[a-z0-9-]*[a-z0-9]:(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21-\x5a\x53-\x7f]|\\[\x01-\x09\x0b\x0c\x0e-\x7f])+)\])"#
);
pub type Email = Refinement<String, Regex<EmailRegex>>;

pub type Password = Refinement<String, ClosedInterval<6, 128>>;

pub type OrgName = Refinement<String, ClosedInterval<1, 64>>;

pub type CheckName = OrgName;

pub type U32InRange<const MIN: usize, const MAX: usize> = Refinement<u32, ClosedInterval<MIN, MAX>>;
