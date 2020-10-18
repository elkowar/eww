use anyhow::*;
use derive_more;
use lazy_static::lazy_static;
use ref_cast::RefCast;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt};

use crate::impl_many;

#[derive(Clone, PartialEq, Deserialize, Serialize, derive_more::From)]
pub struct PrimitiveValue(String);

impl fmt::Display for PrimitiveValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\"{}\"", self.0)
    }
}
impl fmt::Debug for PrimitiveValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::str::FromStr for PrimitiveValue {
    type Err = anyhow::Error;

    /// parses the value, trying to turn it into a number and a boolean first,
    /// before deciding that it is a string.
    fn from_str(s: &str) -> Result<Self> {
        Ok(PrimitiveValue::from_string(s.to_string()))
    }
}

impl_many!(TryFrom<PrimitiveValue> try_from {
    for String => |x| x.as_string();
    for f64 => |x| x.as_f64();
    for bool => |x| x.as_bool();
});

impl From<i32> for PrimitiveValue {
    fn from(x: i32) -> Self {
        PrimitiveValue(format!("{}", x))
    }
}

impl From<bool> for PrimitiveValue {
    fn from(x: bool) -> Self {
        PrimitiveValue(format!("{}", x))
    }
}

impl From<&str> for PrimitiveValue {
    fn from(s: &str) -> Self {
        PrimitiveValue(s.to_string())
    }
}

impl PrimitiveValue {
    pub fn from_string(s: String) -> Self {
        PrimitiveValue(s.to_string())
    }

    /// This will never fail
    pub fn as_string(&self) -> Result<String> {
        Ok(self.0.to_owned())
    }

    pub fn as_f64(&self) -> Result<f64> {
        self.0
            .parse()
            .map_err(|e| anyhow!("couldn't convert {:?} to f64: {}", &self, e))
    }

    pub fn as_i32(&self) -> Result<i32> {
        self.0
            .parse()
            .map_err(|e| anyhow!("couldn't convert {:?} to i32: {}", &self, e))
    }

    pub fn as_bool(&self) -> Result<bool> {
        self.0
            .parse()
            .map_err(|e| anyhow!("couldn't convert {:?} to bool: {}", &self, e))
    }
}

#[repr(transparent)]
#[derive(
    Debug,
    Clone,
    Hash,
    PartialEq,
    Eq,
    derive_more::AsRef,
    derive_more::From,
    derive_more::FromStr,
    Serialize,
    Deserialize,
    RefCast,
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

#[derive(Clone, Debug, PartialEq)]
pub enum AttrValue {
    Concrete(PrimitiveValue),
    VarRef(VarName),
}

impl AttrValue {
    pub fn as_string(&self) -> Result<String> {
        match self {
            AttrValue::Concrete(x) => x.as_string(),
            _ => Err(anyhow!("{:?} is not a string", self)),
        }
    }

    pub fn as_f64(&self) -> Result<f64> {
        match self {
            AttrValue::Concrete(x) => x.as_f64(),
            _ => Err(anyhow!("{:?} is not an f64", self)),
        }
    }

    pub fn as_i32(&self) -> Result<i32> {
        match self {
            AttrValue::Concrete(x) => x.as_i32(),
            _ => Err(anyhow!("{:?} is not an i32", self)),
        }
    }

    pub fn as_bool(&self) -> Result<bool> {
        match self {
            AttrValue::Concrete(x) => x.as_bool(),
            _ => Err(anyhow!("{:?} is not a bool", self)),
        }
    }

    pub fn as_var_ref(&self) -> Result<&VarName> {
        match self {
            AttrValue::VarRef(x) => Ok(x),
            _ => Err(anyhow!("{:?} is not a variable reference", self)),
        }
    }

    /// parses the value, trying to turn it into VarRef,
    /// a number and a boolean first, before deciding that it is a string.
    pub fn parse_string(s: String) -> Self {
        lazy_static! {
            static ref PATTERN: Regex = Regex::new("^\\{\\{(.*)\\}\\}$").unwrap();
        };

        if let Some(ref_name) = PATTERN.captures(&s).and_then(|cap| cap.get(1)).map(|x| x.as_str()) {
            AttrValue::VarRef(VarName(ref_name.to_owned()))
        } else {
            AttrValue::Concrete(PrimitiveValue::from_string(s))
        }
    }
}
impl From<PrimitiveValue> for AttrValue {
    fn from(value: PrimitiveValue) -> Self {
        AttrValue::Concrete(value)
    }
}
