use crate::ast::Span;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt, iter::FromIterator};

pub type Result<T> = std::result::Result<T, ConversionError>;

#[derive(Debug, thiserror::Error)]
#[error("Failed to turn {value} into a {target_type}")]
pub struct ConversionError {
    pub value: DynVal,
    pub target_type: &'static str,
    pub source: Option<Box<dyn std::error::Error>>,
}

impl ConversionError {
    fn new(value: DynVal, target_type: &'static str, source: Box<dyn std::error::Error>) -> Self {
        ConversionError { value, target_type, source: Some(source) }
    }

    pub fn span(&self) -> Option<Span> {
        self.value.1
    }
}

#[derive(Clone, Deserialize, Serialize, Default, Eq)]
pub struct DynVal(pub String, pub Option<Span>);

impl From<String> for DynVal {
    fn from(s: String) -> Self {
        DynVal(s, None)
    }
}

impl fmt::Display for DynVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\"{}\"", self.0)
    }
}
impl fmt::Debug for DynVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\"{}\"", self.0)
    }
}

/// Manually implement equality, to allow for values in different formats (i.e. "1" and "1.0") to still be considered as equal.
impl std::cmp::PartialEq<Self> for DynVal {
    fn eq(&self, other: &Self) -> bool {
        if let (Ok(a), Ok(b)) = (self.as_f64(), other.as_f64()) {
            a == b
        } else {
            self.0 == other.0
        }
    }
}

impl FromIterator<DynVal> for DynVal {
    fn from_iter<T: IntoIterator<Item = DynVal>>(iter: T) -> Self {
        DynVal(iter.into_iter().join(""), None)
    }
}

impl std::str::FromStr for DynVal {
    type Err = ConversionError;

    /// parses the value, trying to turn it into a number and a boolean first,
    /// before deciding that it is a string.
    fn from_str(s: &str) -> Result<Self> {
        Ok(DynVal::from_string(s.to_string()))
    }
}

macro_rules! impl_try_from {
    (impl From<$typ:ty> {
        $(for $for:ty => |$arg:ident| $code:expr);*;
    }) => {
        $(impl TryFrom<$typ> for $for {
            type Error = ConversionError;
            fn try_from($arg: $typ) -> std::result::Result<Self, Self::Error> { $code }
        })*
    };
}
macro_rules! impl_primval_from {
    ($($t:ty),*) => {
        $(impl From<$t> for DynVal {
            fn from(x: $t) -> Self { DynVal(x.to_string(), None) }
        })*
    };
}
impl_try_from!(impl From<DynVal> {
    for String => |x| x.as_string();
    for f64 => |x| x.as_f64();
    for i32 => |x| x.as_i32();
    for bool => |x| x.as_bool();
    //for Vec<String> => |x| x.as_vec();
});

impl_primval_from!(bool, i32, u32, f32, u8, f64, &str);

impl From<&serde_json::Value> for DynVal {
    fn from(v: &serde_json::Value) -> Self {
        DynVal(
            v.as_str()
                .map(|x| x.to_string())
                .or_else(|| serde_json::to_string(v).ok())
                .unwrap_or_else(|| "<invalid json value>".to_string()),
            None,
        )
    }
}

impl DynVal {
    pub fn at(self, span: Span) -> Self {
        DynVal(self.0, Some(span))
    }

    pub fn from_string(s: String) -> Self {
        DynVal(s, None)
    }

    pub fn into_inner(self) -> String {
        self.0
    }

    /// This will never fail
    pub fn as_string(&self) -> Result<String> {
        Ok(self.0.to_owned())
    }

    pub fn as_f64(&self) -> Result<f64> {
        self.0.parse().map_err(|e| ConversionError::new(self.clone(), "f64", Box::new(e)))
    }

    pub fn as_i32(&self) -> Result<i32> {
        self.0.parse().map_err(|e| ConversionError::new(self.clone(), "i32", Box::new(e)))
    }

    pub fn as_bool(&self) -> Result<bool> {
        self.0.parse().map_err(|e| ConversionError::new(self.clone(), "bool", Box::new(e)))
    }

    // pub fn as_vec(&self) -> Result<Vec<String>> {
    // match self.0.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
    // Some(content) => {
    // let mut items: Vec<String> = content.split(',').map(|x: &str| x.to_string()).collect();
    // let mut removed = 0;
    // for times_ran in 0..items.len() {
    //// escapes `,` if there's a `\` before em
    // if items[times_ran - removed].ends_with('\\') {
    // items[times_ran - removed].pop();
    // let it = items.remove((times_ran + 1) - removed);
    // items[times_ran - removed] += ",";
    // items[times_ran - removed] += &it;
    // removed += 1;
    //}
    // Ok(items)
    //}
    // None => Err(ConversionError { value: self.clone(), target_type: "vec", source: None }),
    //}

    pub fn as_json_value(&self) -> Result<serde_json::Value> {
        serde_json::from_str::<serde_json::Value>(&self.0)
            .map_err(|e| ConversionError::new(self.clone(), "json-value", Box::new(e)))
    }
}

#[cfg(test)]
mod test {
    // use super::*;
    // use pretty_assertions::assert_eq;
    //#[test]
    // fn test_parse_vec() {
    // assert_eq!(vec![""], parse_vec("[]".to_string()).unwrap(), "should be able to parse empty lists");
    // assert_eq!(vec!["hi"], parse_vec("[hi]".to_string()).unwrap(), "should be able to parse single element list");
    // assert_eq!(
    // vec!["hi", "ho", "hu"],
    // parse_vec("[hi,ho,hu]".to_string()).unwrap(),
    //"should be able to parse three element list"
    //);
    // assert_eq!(vec!["hi,ho"], parse_vec("[hi\\,ho]".to_string()).unwrap(), "should be able to parse list with escaped comma");
    // assert_eq!(
    // vec!["hi,ho", "hu"],
    // parse_vec("[hi\\,ho,hu]".to_string()).unwrap(),
    //"should be able to parse two element list with escaped comma"
    //);
    // assert!(parse_vec("".to_string()).is_err(), "Should fail when parsing empty string");
    // assert!(parse_vec("[a,b".to_string()).is_err(), "Should fail when parsing unclosed list");
    // assert!(parse_vec("a]".to_string()).is_err(), "Should fail when parsing unopened list");
    //}
}
