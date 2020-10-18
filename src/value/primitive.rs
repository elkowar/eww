use anyhow::*;
use derive_more;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt, iter::FromIterator};

use crate::impl_try_from;

#[derive(Clone, PartialEq, Deserialize, Serialize, derive_more::From, Default)]
pub struct PrimitiveValue(String);

impl fmt::Display for PrimitiveValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl fmt::Debug for PrimitiveValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\"{}\"", self)
    }
}

impl FromIterator<PrimitiveValue> for PrimitiveValue {
    fn from_iter<T: IntoIterator<Item = PrimitiveValue>>(iter: T) -> Self {
        PrimitiveValue(iter.into_iter().join(""))
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

impl_try_from!(PrimitiveValue {
    for String => |x| x.as_string();
    for f64 => |x| x.as_f64();
    for bool => |x| x.as_bool();
});

impl From<bool> for PrimitiveValue {
    fn from(x: bool) -> Self {
        PrimitiveValue(x.to_string())
    }
}

impl From<i32> for PrimitiveValue {
    fn from(s: i32) -> Self {
        PrimitiveValue(s.to_string())
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

    pub fn into_inner(self) -> String {
        self.0
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
