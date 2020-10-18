use derive_more;
use ref_cast::RefCast;
use serde::{Deserialize, Serialize};
use std::fmt;

pub mod attr_value;
pub mod primitive;
pub mod string_with_varrefs;
pub use attr_value::*;
pub use primitive::*;
pub use string_with_varrefs::*;

#[repr(transparent)]
#[derive(
    Clone, Hash, PartialEq, Eq, derive_more::AsRef, derive_more::From, derive_more::FromStr, Serialize, Deserialize, RefCast,
)]
pub struct VarName(pub String);

impl std::borrow::Borrow<str> for VarName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VarName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for VarName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VarName(\"{}\")", self.0)
    }
}

#[repr(transparent)]
#[derive(
    Clone, Hash, PartialEq, Eq, derive_more::AsRef, derive_more::From, derive_more::FromStr, Serialize, Deserialize, RefCast,
)]
pub struct AttrName(pub String);

impl std::borrow::Borrow<str> for AttrName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AttrName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for AttrName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AttrName({})", self.0)
    }
}
