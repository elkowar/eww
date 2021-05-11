use anyhow::*;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt, iter::FromIterator};

use crate::impl_try_from;

#[derive(Clone, Deserialize, Serialize, derive_more::From, Default)]
pub struct PrimVal(pub String);

impl fmt::Display for PrimVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl fmt::Debug for PrimVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\"{}\"", self.0)
    }
}

/// Manually implement equality, to allow for values in different formats (i.e. "1" and "1.0") to still be considered as equal.
impl std::cmp::PartialEq<Self> for PrimVal {
    fn eq(&self, other: &Self) -> bool {
        if let (Ok(a), Ok(b)) = (self.as_f64(), other.as_f64()) {
            a == b
        } else {
            self.0 == other.0
        }
    }
}

impl FromIterator<PrimVal> for PrimVal {
    fn from_iter<T: IntoIterator<Item = PrimVal>>(iter: T) -> Self {
        PrimVal(iter.into_iter().join(""))
    }
}

impl std::str::FromStr for PrimVal {
    type Err = anyhow::Error;

    /// parses the value, trying to turn it into a number and a boolean first,
    /// before deciding that it is a string.
    fn from_str(s: &str) -> Result<Self> {
        Ok(PrimVal::from_string(s.to_string()))
    }
}

impl_try_from!(PrimVal {
    for String => |x| x.as_string();
    for f64 => |x| x.as_f64();
    for bool => |x| x.as_bool();
    for Vec<String> => |x| x.as_vec();
});

impl From<bool> for PrimVal {
    fn from(x: bool) -> Self {
        PrimVal(x.to_string())
    }
}

impl From<i32> for PrimVal {
    fn from(s: i32) -> Self {
        PrimVal(s.to_string())
    }
}

impl From<u32> for PrimVal {
    fn from(s: u32) -> Self {
        PrimVal(s.to_string())
    }
}

impl From<f32> for PrimVal {
    fn from(s: f32) -> Self {
        PrimVal(s.to_string())
    }
}

impl From<u8> for PrimVal {
    fn from(s: u8) -> Self {
        PrimVal(s.to_string())
    }
}
impl From<f64> for PrimVal {
    fn from(s: f64) -> Self {
        PrimVal(s.to_string())
    }
}

impl From<&str> for PrimVal {
    fn from(s: &str) -> Self {
        PrimVal(s.to_string())
    }
}

impl From<&serde_json::Value> for PrimVal {
    fn from(v: &serde_json::Value) -> Self {
        PrimVal(
            v.as_str()
                .map(|x| x.to_string())
                .or_else(|| serde_json::to_string(v).ok())
                .unwrap_or_else(|| "<invalid json value>".to_string()),
        )
    }
}

impl PrimVal {
    pub fn from_string(s: String) -> Self {
        PrimVal(s)
    }

    pub fn into_inner(self) -> String {
        self.0
    }

    /// This will never fail
    pub fn as_string(&self) -> Result<String> {
        Ok(self.0.to_owned())
    }

    pub fn as_f64(&self) -> Result<f64> {
        self.0.parse().map_err(|e| anyhow!("couldn't convert {:?} to f64: {}", &self, e))
    }

    pub fn as_i32(&self) -> Result<i32> {
        self.0.parse().map_err(|e| anyhow!("couldn't convert {:?} to i32: {}", &self, e))
    }

    pub fn as_bool(&self) -> Result<bool> {
        self.0.parse().map_err(|e| anyhow!("couldn't convert {:?} to bool: {}", &self, e))
    }

    pub fn as_vec(&self) -> Result<Vec<String>> {
        parse_vec(self.0.to_owned()).map_err(|e| anyhow!("Couldn't convert {:#?} to a vec: {}", &self, e))
    }

    pub fn as_json_value(&self) -> Result<serde_json::Value> {
        serde_json::from_str::<serde_json::Value>(&self.0)
            .with_context(|| format!("Couldn't convert {:#?} to a json object", &self))
    }
}

fn parse_vec(a: String) -> Result<Vec<String>> {
    match a.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
        Some(content) => {
            let mut items: Vec<String> = content.split(',').map(|x: &str| x.to_string()).collect();
            let mut removed = 0;
            for times_ran in 0..items.len() {
                // escapes `,` if there's a `\` before em
                if items[times_ran - removed].ends_with('\\') {
                    items[times_ran - removed].pop();
                    let it = items.remove((times_ran + 1) - removed);
                    items[times_ran - removed] += ",";
                    items[times_ran - removed] += &it;
                    removed += 1;
                }
            }
            Ok(items)
        }
        None => Err(anyhow!("Is your array built like this: '[these,are,items]'?")),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    #[test]
    fn test_parse_vec() {
        assert_eq!(vec![""], parse_vec("[]".to_string()).unwrap(), "should be able to parse empty lists");
        assert_eq!(vec!["hi"], parse_vec("[hi]".to_string()).unwrap(), "should be able to parse single element list");
        assert_eq!(
            vec!["hi", "ho", "hu"],
            parse_vec("[hi,ho,hu]".to_string()).unwrap(),
            "should be able to parse three element list"
        );
        assert_eq!(vec!["hi,ho"], parse_vec("[hi\\,ho]".to_string()).unwrap(), "should be able to parse list with escaped comma");
        assert_eq!(
            vec!["hi,ho", "hu"],
            parse_vec("[hi\\,ho,hu]".to_string()).unwrap(),
            "should be able to parse two element list with escaped comma"
        );
        assert!(parse_vec("".to_string()).is_err(), "Should fail when parsing empty string");
        assert!(parse_vec("[a,b".to_string()).is_err(), "Should fail when parsing unclosed list");
        assert!(parse_vec("a]".to_string()).is_err(), "Should fail when parsing unopened list");
    }
}
