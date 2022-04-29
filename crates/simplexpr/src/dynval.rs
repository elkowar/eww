use eww_shared_util::{Span, Spanned};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{convert::TryFrom, fmt, str::FromStr};

pub type Result<T> = std::result::Result<T, ConversionError>;

#[derive(Debug, thiserror::Error)]
#[error("Failed to turn `{value}` into a value of type {target_type}")]
pub struct ConversionError {
    pub value: DynVal,
    pub target_type: &'static str,
    pub source: Option<Box<dyn std::error::Error + Sync + Send + 'static>>,
}

impl ConversionError {
    pub fn new(value: DynVal, target_type: &'static str, source: impl std::error::Error + 'static + Sync + Send) -> Self {
        ConversionError { value, target_type, source: Some(Box::new(source)) }
    }

    pub fn no_source(value: DynVal, target_type: &'static str) -> Self {
        ConversionError { value, target_type, source: None }
    }
}
impl Spanned for ConversionError {
    fn span(&self) -> Span {
        self.value.span()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Opaque {
    pub type_name: String,
    pub value: serde_json::Value,
}

pub trait OpaqueType: DeserializeOwned + serde::Serialize {
    const TYPE_NAME: &'static str;
    fn from_opaque(opaque: Opaque) -> Result<Self> {
        if opaque.type_name != Self::TYPE_NAME {
            // TODO the DUMMY here is kinda ugly
            Err(ConversionError::no_source(DynVal::Opaque(Span::DUMMY, opaque), Self::TYPE_NAME))
        } else {
            Ok(serde_json::from_value::<Self>(opaque.value.clone())
                .map_err(|e| ConversionError::new(DynVal::Opaque(Span::DUMMY, opaque), Self::TYPE_NAME, e))?)
        }
    }
    fn to_opaque(self) -> Opaque {
        Opaque { type_name: Self::TYPE_NAME.to_string(), value: serde_json::to_value(self).unwrap() }
    }
}

#[derive(Clone, Deserialize, Serialize, Eq)]
pub enum DynVal {
    Value(Span, String),
    Opaque(Span, Opaque),
}

impl From<String> for DynVal {
    fn from(s: String) -> Self {
        DynVal::Value(Span::DUMMY, s)
    }
}

impl fmt::Display for DynVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DynVal::Value(_, x) => write!(f, "{}", x),
            DynVal::Opaque(_, x) => write!(f, "<{}>", x.type_name),
        }
    }
}
impl fmt::Debug for DynVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DynVal::Value(_, x) => write!(f, "\"{}\"", x),
            DynVal::Opaque(_, x) => write!(f, "<{}>{:?}", x.type_name, x.value),
        }
    }
}

/// Manually implement equality, to allow for values in different formats (i.e. "1" and "1.0") to still be considered as equal.
/// Spans are ignored in equality here
impl std::cmp::PartialEq<Self> for DynVal {
    fn eq(&self, other: &Self) -> bool {
        if let (Ok(a), Ok(b)) = (self.as_f64(), other.as_f64()) {
            a == b
        } else {
            match (self, other) {
                (Self::Value(_, a), Self::Value(_, b)) => a == b,
                (Self::Opaque(_, a), Self::Opaque(_, b)) => a == b,
                _ => false,
            }
        }
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

impl<E: std::error::Error + Sync + Send + 'static, T: FromStr<Err = E>> FromDynVal for T {
    type Err = ConversionError;

    fn from_dynval(x: &DynVal) -> std::result::Result<Self, Self::Err> {
        match x {
            DynVal::Value(_, s) => T::from_str(s).map_err(|e| ConversionError::new(x.clone(), std::any::type_name::<T>(), e)),
            DynVal::Opaque(..) => Err(ConversionError::no_source(x.clone(), std::any::type_name::<T>())),
        }
    }
}

macro_rules! impl_dynval_from {
    ($($t:ty),*) => {
        $(impl From<$t> for DynVal {
            fn from(x: $t) -> Self { DynVal::Value(Span::DUMMY, x.to_string()) }
        })*
    };
}

impl_dynval_from!(bool, i32, u32, f32, u8, f64, &str);

impl TryFrom<serde_json::Value> for DynVal {
    type Error = serde_json::Error;

    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        if let Ok(opaque) = serde_json::from_value::<Opaque>(value.clone()) {
            Ok(DynVal::Opaque(Span::DUMMY, opaque))
        } else {
            Ok(DynVal::Value(Span::DUMMY, serde_json::to_string(&value)?))
        }
    }
}

impl From<std::time::Duration> for DynVal {
    fn from(d: std::time::Duration) -> Self {
        DynVal::Value(Span::DUMMY, format!("{}ms", d.as_millis()))
    }
}

impl From<&serde_json::Value> for DynVal {
    fn from(v: &serde_json::Value) -> Self {
        DynVal::Value(
            Span::DUMMY,
            v.as_str()
                .map(|x| x.to_string())
                .or_else(|| serde_json::to_string(v).ok())
                .unwrap_or_else(|| "<invalid json value>".to_string()),
        )
    }
}

impl Spanned for DynVal {
    fn span(&self) -> Span {
        match self {
            DynVal::Value(span, _) => *span,
            DynVal::Opaque(span, ..) => *span,
        }
    }
}

impl DynVal {
    /// Succeeds with the raw string in the dynval if the dynval is a [`DynVal::Value`].
    /// Fails with a [`ConversionError`] otherwise.
    fn try_as_value_for(&self, target_type: &'static str) -> Result<&str> {
        match self {
            DynVal::Value(_, s) => Ok(s),
            DynVal::Opaque(..) => Err(ConversionError::no_source(self.clone(), target_type)),
        }
    }

    pub fn as_opaque<T: OpaqueType>(&self) -> Result<T> {
        match self {
            DynVal::Opaque(_, opaque) => Ok(T::from_opaque(opaque.clone())?),
            DynVal::Value(..) => Err(ConversionError::no_source(self.clone(), T::TYPE_NAME)),
        }
    }

    pub fn at(mut self, span: Span) -> Self {
        match self {
            DynVal::Value(ref mut s, _) => *s = span,
            DynVal::Opaque(ref mut s, ..) => *s = span,
        }
        self
    }

    pub fn is_nullish(&self) -> bool {
        match self {
            DynVal::Value(_, s) => s.is_empty() || s == "null",
            DynVal::Opaque(..) => false,
        }
    }

    pub fn from_string(s: String) -> Self {
        DynVal::Value(Span::DUMMY, s)
    }

    pub fn read_as<E, T: FromDynVal<Err = E>>(&self) -> std::result::Result<T, E> {
        T::from_dynval(self)
    }

    pub fn as_string(&self) -> Result<String> {
        self.try_as_value_for("string").map(|s| s.to_string())
    }

    pub fn as_f64(&self) -> Result<f64> {
        self.try_as_value_for("f64")?.parse().map_err(|e| ConversionError::new(self.clone(), "f64", e))
    }

    pub fn as_i32(&self) -> Result<i32> {
        self.try_as_value_for("i32")?.parse().map_err(|e| ConversionError::new(self.clone(), "i32", e))
    }

    pub fn as_bool(&self) -> Result<bool> {
        self.try_as_value_for("bool")?.parse().map_err(|e| ConversionError::new(self.clone(), "bool", e))
    }

    pub fn as_duration(&self) -> Result<std::time::Duration> {
        use std::time::Duration;
        let s = &self.try_as_value_for("duration")?.to_string();
        if s.ends_with("ms") {
            Ok(Duration::from_millis(
                s.trim_end_matches("ms").parse().map_err(|e| ConversionError::new(self.clone(), "integer", e))?,
            ))
        } else if s.ends_with('s') {
            Ok(Duration::from_secs(
                s.trim_end_matches('s').parse().map_err(|e| ConversionError::new(self.clone(), "integer", e))?,
            ))
        } else if s.ends_with('m') {
            Ok(Duration::from_secs(
                s.trim_end_matches('m').parse::<u64>().map_err(|e| ConversionError::new(self.clone(), "integer", e))? * 60,
            ))
        } else if s.ends_with('h') {
            Ok(Duration::from_secs(
                s.trim_end_matches('h').parse::<u64>().map_err(|e| ConversionError::new(self.clone(), "integer", e))? * 60 * 60,
            ))
        } else {
            Err(ConversionError { value: self.clone(), target_type: "duration", source: None })
        }
    }

    pub fn as_json_value(&self) -> Result<serde_json::Value> {
        serde_json::from_str::<serde_json::Value>(&self.try_as_value_for("json")?)
            .map_err(|e| ConversionError::new(self.clone(), "json-value", Box::new(e)))
    }

    pub fn as_json_array(&self) -> Result<Vec<serde_json::Value>> {
        serde_json::from_str::<serde_json::Value>(&self.try_as_value_for("json-array")?)
            .map_err(|e| ConversionError::new(self.clone(), "json-array", Box::new(e)))?
            .as_array()
            .cloned()
            .ok_or_else(|| ConversionError::no_source(self.clone(), "json-array"))
    }

    pub fn as_json_object(&self) -> Result<serde_json::Map<String, serde_json::Value>> {
        serde_json::from_str::<serde_json::Value>(&self.try_as_value_for("json-object")?)
            .map_err(|e| ConversionError::new(self.clone(), "json-object", Box::new(e)))?
            .as_object()
            .cloned()
            .ok_or_else(|| ConversionError::no_source(self.clone(), "json-object"))
    }
}
