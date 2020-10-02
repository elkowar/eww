use anyhow::*;
use derive_more;
use hocon::Hocon;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use try_match::try_match;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, derive_more::From)]
pub enum PrimitiveValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct PollingCommandValue {
    command: String,
    interval: std::time::Duration,
}

impl std::str::FromStr for PrimitiveValue {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<PrimitiveValue> {
        Ok(s.parse()
            .map(PrimitiveValue::Number)
            .or_else(|_| s.parse().map(PrimitiveValue::Boolean))
            .unwrap_or_else(|_| PrimitiveValue::String(remove_surrounding(s, '\'').to_string())))
    }
}

fn remove_surrounding(s: &str, surround: char) -> &str {
    s.strip_prefix(surround).unwrap_or(s).strip_suffix(surround).unwrap_or(s)
}

impl TryFrom<PrimitiveValue> for String {
    type Error = anyhow::Error;
    fn try_from(x: PrimitiveValue) -> Result<Self> {
        x.as_string()
    }
}

impl TryFrom<PrimitiveValue> for f64 {
    type Error = anyhow::Error;
    fn try_from(x: PrimitiveValue) -> Result<Self> {
        x.as_f64()
    }
}

impl TryFrom<PrimitiveValue> for bool {
    type Error = anyhow::Error;
    fn try_from(x: PrimitiveValue) -> Result<Self> {
        x.as_bool()
    }
}

impl From<&str> for PrimitiveValue {
    fn from(s: &str) -> Self {
        PrimitiveValue::String(s.to_string())
    }
}

impl PrimitiveValue {
    pub fn as_string(&self) -> Result<String> {
        match self {
            PrimitiveValue::String(x) => Ok(x.clone()),
            PrimitiveValue::Number(x) => Ok(format!("{}", x)),
            PrimitiveValue::Boolean(x) => Ok(format!("{}", x)),
        }
    }
    pub fn as_f64(&self) -> Result<f64> {
        match self {
            PrimitiveValue::Number(x) => Ok(*x),
            PrimitiveValue::String(x) => x
                .parse()
                .map_err(|e| anyhow!("couldn't convert string {:?} to f64: {}", &self, e)),
            _ => Err(anyhow!("{:?} is not an f64", &self)),
        }
    }
    pub fn as_bool(&self) -> Result<bool> {
        match self {
            PrimitiveValue::Boolean(x) => Ok(*x),
            PrimitiveValue::String(x) => x
                .parse()
                .map_err(|e| anyhow!("couldn't convert string {:?} to bool: {}", &self, e)),
            _ => Err(anyhow!("{:?} is not a string", &self)),
        }
    }
}

impl std::convert::TryFrom<&Hocon> for PrimitiveValue {
    type Error = anyhow::Error;
    fn try_from(value: &Hocon) -> Result<Self> {
        Ok(match value {
            Hocon::String(s) if s.starts_with("$$") => {
                return Err(anyhow!("Tried to use variable reference {} as primitive value", s))
            }
            Hocon::String(s) => PrimitiveValue::String(s.to_string()),
            Hocon::Integer(n) => PrimitiveValue::Number(*n as f64),
            Hocon::Real(n) => PrimitiveValue::Number(*n as f64),
            Hocon::Boolean(b) => PrimitiveValue::Boolean(*b),
            _ => return Err(anyhow!("cannot convert {} to config::ConcreteValue")),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AttrValue {
    Concrete(PrimitiveValue),
    VarRef(String),
    CommandPolling(CommandPollingUse),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommandPollingUse {
    pub command: String,
    pub interval: std::time::Duration,
}

impl AttrValue {
    pub fn as_string(&self) -> Result<String> {
        try_match!(AttrValue::Concrete(x) = self)
            .map_err(|e| anyhow!("{:?} is not a string", e))?
            .as_string()
    }
    pub fn as_f64(&self) -> Result<f64> {
        try_match!(AttrValue::Concrete(x) = self)
            .map_err(|e| anyhow!("{:?} is not an f64", e))?
            .as_f64()
    }
    pub fn as_bool(&self) -> Result<bool> {
        try_match!(AttrValue::Concrete(x) = self)
            .map_err(|e| anyhow!("{:?} is not a bool", e))?
            .as_bool()
    }
    pub fn as_var_ref(&self) -> Result<&String> {
        try_match!(AttrValue::VarRef(x) = self).map_err(|e| anyhow!("{:?} is not a VarRef", e))
    }

    pub fn from_string(s: String) -> Self {
        if s.starts_with("$$") {
            AttrValue::VarRef(s.trim_start_matches("$$").to_string())
        } else {
            AttrValue::Concrete(PrimitiveValue::String(s.clone()))
        }
    }
}
impl From<PrimitiveValue> for AttrValue {
    fn from(value: PrimitiveValue) -> Self {
        AttrValue::Concrete(value)
    }
}

impl std::convert::TryFrom<&Hocon> for AttrValue {
    type Error = anyhow::Error;
    fn try_from(value: &Hocon) -> Result<Self> {
        Ok(match value {
            Hocon::String(s) => AttrValue::from_string(s.clone()),
            Hocon::Integer(n) => AttrValue::Concrete(PrimitiveValue::Number(*n as f64)),
            Hocon::Real(n) => AttrValue::Concrete(PrimitiveValue::Number(*n as f64)),
            Hocon::Boolean(b) => AttrValue::Concrete(PrimitiveValue::Boolean(*b)),
            _ => return Err(anyhow!("cannot convert {:?} to config::AttrValue", &value)),
        })
    }
}
