use derive_more::*;
use serde::{Deserialize, Serialize};

pub mod attr_value;
pub mod coords;
pub mod primitive;
pub use attr_value::*;
pub use attr_value_expr::*;
pub use coords::*;
pub use primitive::*;

/// The name of a variable
#[repr(transparent)]
#[derive(Clone, Hash, PartialEq, Eq, Serialize, Deserialize, AsRef, From, FromStr, Display, DebugCustom)]
#[debug(fmt = "VarName({})", .0)]
pub struct VarName(pub String);

impl std::borrow::Borrow<str> for VarName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl From<&str> for VarName {
    fn from(s: &str) -> Self {
        VarName(s.to_owned())
    }
}

/// The name of an attribute
#[repr(transparent)]
#[derive(Clone, Hash, PartialEq, Eq, Serialize, Deserialize, AsRef, From, FromStr, Display, DebugCustom)]
#[debug(fmt="AttrName({})", .0)]
pub struct AttrName(pub String);

impl std::borrow::Borrow<str> for AttrName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl From<&str> for AttrName {
    fn from(s: &str) -> Self {
        AttrName(s.to_owned())
    }
}
