use eww_shared_util::{Span, Spanned};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt, iter::FromIterator, str::FromStr};

pub type Result<T> = std::result::Result<T, ConversionError>;

#[derive(Debug, thiserror::Error)]
#[error("Failed to turn `{value}` into a value of type {target_type}")]
pub struct ConversionError {
    pub value: DynVal,
    pub target_type: &'static str,
    pub source: Option<Box<dyn std::error::Error + Sync + Send + 'static>>,
}

#[derive(Debug, thiserror::Error)]
#[error("Failed to parse duration. Must be a number of milliseconds, or a string like \"150ms\"")]
pub struct DurationParseError;

impl ConversionError {
    pub fn new(value: DynVal, target_type: &'static str, source: impl std::error::Error + 'static + Sync + Send) -> Self {
        ConversionError { value, target_type, source: Some(Box::new(source)) }
    }
}
impl Spanned for ConversionError {
    fn span(&self) -> Span {
        self.value.1
    }
}

#[derive(Clone, Deserialize, Serialize, Eq)]
pub struct DynVal(pub String, pub Span);

impl From<String> for DynVal {
    fn from(s: String) -> Self {
        DynVal(s, Span::DUMMY)
    }
}

impl fmt::Display for DynVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
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
        DynVal(iter.into_iter().join(""), Span::DUMMY)
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

pub trait FromDynVal: Sized {
    type Err;
    fn from_dynval(x: &DynVal) -> std::result::Result<Self, Self::Err>;
}

impl<E, T: FromStr<Err = E>> FromDynVal for T {
    type Err = E;

    fn from_dynval(x: &DynVal) -> std::result::Result<Self, Self::Err> {
        x.0.parse()
    }
}

macro_rules! impl_dynval_from {
    ($($t:ty),*) => {
        $(impl From<$t> for DynVal {
            fn from(x: $t) -> Self { DynVal(x.to_string(), Span::DUMMY) }
        })*
    };
}

impl_dynval_from!(bool, i32, u32, f32, u8, f64, &str);

impl TryFrom<serde_json::Value> for DynVal {
    type Error = serde_json::Error;

    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        Ok(DynVal(serde_json::to_string(&value)?, Span::DUMMY))
    }
}

impl From<Vec<DynVal>> for DynVal {
    fn from(v: Vec<DynVal>) -> Self {
        let span = if let (Some(first), Some(last)) = (v.first(), v.last()) { first.span().to(last.span()) } else { Span::DUMMY };
        let elements = v.into_iter().map(|x| x.as_string().unwrap()).collect::<Vec<_>>();
        DynVal(serde_json::to_string(&elements).unwrap(), span)
    }
}

impl From<std::time::Duration> for DynVal {
    fn from(d: std::time::Duration) -> Self {
        DynVal(format!("{}ms", d.as_millis()), Span::DUMMY)
    }
}

impl From<&serde_json::Value> for DynVal {
    fn from(v: &serde_json::Value) -> Self {
        DynVal(
            v.as_str()
                .map(|x| x.to_string())
                .or_else(|| serde_json::to_string(v).ok())
                .unwrap_or_else(|| "<invalid json value>".to_string()),
            Span::DUMMY,
        )
    }
}

impl Spanned for DynVal {
    fn span(&self) -> Span {
        self.1
    }
}

impl DynVal {
    pub fn at(mut self, span: Span) -> Self {
        self.1 = span;
        self
    }

    pub fn at_if_dummy(mut self, span: Span) -> Self {
        if self.1.is_dummy() {
            self.1 = span;
        }
        self
    }

    pub fn from_string(s: String) -> Self {
        DynVal(s, Span::DUMMY)
    }

    pub fn read_as<E, T: FromDynVal<Err = E>>(&self) -> std::result::Result<T, E> {
        T::from_dynval(self)
    }

    pub fn into_inner(self) -> String {
        self.0
    }

    /// This will never fail
    pub fn as_string(&self) -> Result<String> {
        Ok(self.0.to_owned())
    }

    pub fn as_f64(&self) -> Result<f64> {
        self.0.parse().map_err(|e| ConversionError::new(self.clone(), "f64", e))
    }

    pub fn as_i32(&self) -> Result<i32> {
        self.0.parse().map_err(|e| ConversionError::new(self.clone(), "i32", e))
    }

    pub fn as_i64(&self) -> Result<i64> {
        self.0.parse().map_err(|e| ConversionError::new(self.clone(), "i64", e))
    }

    pub fn as_bool(&self) -> Result<bool> {
        self.0.parse().map_err(|e| ConversionError::new(self.clone(), "bool", e))
    }

    pub fn as_duration(&self) -> Result<std::time::Duration> {
        use std::time::Duration;
        let s = &self.0;
        if s.ends_with("ms") {
            Ok(Duration::from_millis(
                s.trim_end_matches("ms").parse().map_err(|e| ConversionError::new(self.clone(), "integer", e))?,
            ))
        } else if s.ends_with('s') {
            let secs = s.trim_end_matches('s').parse::<f64>().map_err(|e| ConversionError::new(self.clone(), "number", e))?;
            Ok(Duration::from_millis(f64::floor(secs * 1000f64) as u64))
        } else if s.ends_with('m') || s.ends_with("min") {
            let minutes = s
                .trim_end_matches("min")
                .trim_end_matches('m')
                .parse::<f64>()
                .map_err(|e| ConversionError::new(self.clone(), "number", e))?;
            Ok(Duration::from_secs(f64::floor(minutes * 60f64) as u64))
        } else if s.ends_with('h') {
            let hours = s.trim_end_matches('h').parse::<f64>().map_err(|e| ConversionError::new(self.clone(), "number", e))?;
            Ok(Duration::from_secs(f64::floor(hours * 60f64 * 60f64) as u64))
        } else if let Ok(millis) = s.parse() {
            Ok(Duration::from_millis(millis))
        } else {
            Err(ConversionError { value: self.clone(), target_type: "duration", source: Some(Box::new(DurationParseError)) })
        }
    }

    // TODO this should return Result<Vec<DynVal>> and use json parsing
    pub fn as_vec(&self) -> Result<Vec<String>> {
        if self.0.is_empty() {
            Ok(Vec::new())
        } else {
            match self.0.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
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
                None => Err(ConversionError { value: self.clone(), target_type: "vec", source: None }),
            }
        }
    }

    pub fn as_json_value(&self) -> Result<serde_json::Value> {
        serde_json::from_str::<serde_json::Value>(&self.0)
            .map_err(|e| ConversionError::new(self.clone(), "json-value", Box::new(e)))
    }

    pub fn as_json_array(&self) -> Result<Vec<serde_json::Value>> {
        serde_json::from_str::<serde_json::Value>(&self.0)
            .map_err(|e| ConversionError::new(self.clone(), "json-value", Box::new(e)))?
            .as_array()
            .cloned()
            .ok_or_else(|| ConversionError { value: self.clone(), target_type: "json-array", source: None })
    }

    pub fn as_json_object(&self) -> Result<serde_json::Map<String, serde_json::Value>> {
        serde_json::from_str::<serde_json::Value>(&self.0)
            .map_err(|e| ConversionError::new(self.clone(), "json-value", Box::new(e)))?
            .as_object()
            .cloned()
            .ok_or_else(|| ConversionError { value: self.clone(), target_type: "json-object", source: None })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_parse_vec() {
        insta::assert_debug_snapshot!(DynVal::from("[]").as_vec());
        insta::assert_debug_snapshot!(DynVal::from("[hi]").as_vec());
        insta::assert_debug_snapshot!(DynVal::from("[hi,ho,hu]").as_vec());
        insta::assert_debug_snapshot!(DynVal::from("[hi\\,ho]").as_vec());
        insta::assert_debug_snapshot!(DynVal::from("[hi\\,ho,hu]").as_vec());
        insta::assert_debug_snapshot!(DynVal::from("").as_vec());
        insta::assert_debug_snapshot!(DynVal::from("[a,b").as_vec());
        insta::assert_debug_snapshot!(DynVal::from("a]").as_vec());
    }

    #[test]
    fn test_parse_duration() {
        insta::assert_debug_snapshot!(DynVal::from("100ms").as_duration());
        insta::assert_debug_snapshot!(DynVal::from("1s").as_duration());
        insta::assert_debug_snapshot!(DynVal::from("0.1s").as_duration());
        insta::assert_debug_snapshot!(DynVal::from("5m").as_duration());
        insta::assert_debug_snapshot!(DynVal::from("5min").as_duration());
        insta::assert_debug_snapshot!(DynVal::from("0.5m").as_duration());
        insta::assert_debug_snapshot!(DynVal::from("1h").as_duration());
        insta::assert_debug_snapshot!(DynVal::from("0.5h").as_duration());
    }
}
