use anyhow::*;
use derive_more;
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;

#[derive(Clone, PartialEq, Deserialize, Serialize, derive_more::From)]
pub enum PrimitiveValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

impl fmt::Display for PrimitiveValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PrimitiveValue::String(s) => write!(f, "\"{}\"", s),
            PrimitiveValue::Number(n) => write!(f, "{}", n),
            PrimitiveValue::Boolean(b) => write!(f, "{}", b),
        }
    }
}
impl fmt::Debug for PrimitiveValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::str::FromStr for PrimitiveValue {
    type Err = anyhow::Error;

    /// parses the value, trying to turn it into a number and a boolean first, before deciding that it is a string.
    fn from_str(s: &str) -> Result<Self> {
        Ok(PrimitiveValue::parse_string(s))
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
    /// parses the value, trying to turn it into a number and a boolean first, before deciding that it is a string.
    pub fn parse_string(s: &str) -> Self {
        s.parse()
            .map(PrimitiveValue::Number)
            .or_else(|_| s.parse().map(PrimitiveValue::Boolean))
            .unwrap_or_else(|_| PrimitiveValue::String(remove_surrounding(s, '\'').to_string()))
    }
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

#[derive(Clone, Debug, PartialEq)]
pub enum AttrValue {
    Concrete(PrimitiveValue),
    VarRef(String),
}

impl AttrValue {
    pub fn as_string(&self) -> Result<String> {
        match self {
            AttrValue::Concrete(x) => Ok(x.as_string()?),
            _ => Err(anyhow!("{:?} is not a string", self)),
        }
    }
    pub fn as_f64(&self) -> Result<f64> {
        match self {
            AttrValue::Concrete(x) => Ok(x.as_f64()?),
            _ => Err(anyhow!("{:?} is not an f64", self)),
        }
    }
    pub fn as_bool(&self) -> Result<bool> {
        match self {
            AttrValue::Concrete(x) => Ok(x.as_bool()?),
            _ => Err(anyhow!("{:?} is not a bool", self)),
        }
    }

    /// parses the value, trying to turn it into VarRef,
    /// a number and a boolean first, before deciding that it is a string.
    pub fn parse_string(s: String) -> Self {
        lazy_static! {
            static ref PATTERN: Regex = Regex::new("^\\{\\{(.*)\\}\\}$").unwrap();
        };

        if let Some(ref_name) = PATTERN.captures(&s).and_then(|cap| cap.get(1)).map(|x| x.as_str()) {
            AttrValue::VarRef(ref_name.to_owned())
        } else {
            AttrValue::Concrete(PrimitiveValue::String(s))
        }
    }
}
impl From<PrimitiveValue> for AttrValue {
    fn from(value: PrimitiveValue) -> Self {
        AttrValue::Concrete(value)
    }
}
