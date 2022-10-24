use derive_more::*;
use ref_cast::RefCast;
use serde::{Deserialize, Serialize};

/// The name of a variable
#[repr(transparent)]
#[derive(Clone, Hash, PartialEq, Eq, Serialize, Deserialize, AsRef, From, FromStr, Display, DebugCustom, RefCast)]
#[debug(fmt = "VarName({})", .0)]
pub struct VarName(pub String);

impl std::borrow::Borrow<str> for VarName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AttrName {
    pub fn to_attr_name_ref(&self) -> &AttrName {
        AttrName::ref_cast(&self.0)
    }
}

impl From<&str> for VarName {
    fn from(s: &str) -> Self {
        VarName(s.to_owned())
    }
}

impl From<AttrName> for VarName {
    fn from(x: AttrName) -> Self {
        VarName(x.0)
    }
}

/// The name of an attribute
#[repr(transparent)]
#[derive(Clone, Hash, PartialEq, Eq, Serialize, Deserialize, AsRef, From, FromStr, Display, DebugCustom, RefCast)]
#[debug(fmt="AttrName({})", .0)]
pub struct AttrName(pub String);

impl AttrName {
    pub fn to_var_name_ref(&self) -> &VarName {
        VarName::ref_cast(&self.0)
    }
}

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

impl From<VarName> for AttrName {
    fn from(x: VarName) -> Self {
        AttrName(x.0)
    }
}
