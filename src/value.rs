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
        match x {
            PrimitiveValue::String(x) => Ok(x),
            _ => return Err(anyhow!("'{:?}' is not a string", x.clone())),
        }
    }
}

impl TryFrom<PrimitiveValue> for f64 {
    type Error = anyhow::Error;
    fn try_from(x: PrimitiveValue) -> Result<Self> {
        try_match!(PrimitiveValue::Number(x) = &x)
            .map_err(|_| anyhow!("'{:?}' is not a number", &x))
            .map(|&x| x)
    }
}

impl TryFrom<PrimitiveValue> for bool {
    type Error = anyhow::Error;
    fn try_from(x: PrimitiveValue) -> Result<Self> {
        try_match!(PrimitiveValue::Boolean(x) = &x)
            .map_err(|_| anyhow!("'{:?}' is not a bool", &x))
            .map(|&x| x)
    }
}

impl From<&str> for PrimitiveValue {
    fn from(s: &str) -> Self {
        PrimitiveValue::String(s.to_string())
    }
}

impl PrimitiveValue {
    pub fn as_string(&self) -> Result<&String> {
        try_match!(PrimitiveValue::String(x) = self).map_err(|x| anyhow!("{:?} is not a string", x))
    }
    pub fn as_f64(&self) -> Result<f64> {
        try_match!(PrimitiveValue::Number(x) = self)
            .map_err(|x| anyhow!("{:?} is not an f64", x))
            .map(|&x| x)
    }
    pub fn as_bool(&self) -> Result<bool> {
        try_match!(PrimitiveValue::Boolean(x) = self)
            .map_err(|x| anyhow!("{:?} is not a bool", x))
            .map(|&x| x)
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
            _ => return Err(anyhow!("cannot convert {} to config::PrimitiveValue")),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AttrValue {
    Concrete(PrimitiveValue),
    VarRef(String),
}

impl AttrValue {
    pub fn as_string(&self) -> Result<&String> {
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
