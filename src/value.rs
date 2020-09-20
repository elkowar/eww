use anyhow::*;
use derive_more::From;
use hocon::Hocon;
use try_match::try_match;

#[derive(Clone, Debug, PartialEq, From)]
pub enum PrimitiveValue {
    String(String),
    Number(f64),
    Boolean(bool),
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
                return Err(anyhow!(
                    "Tried to use variable reference {} as primitive value",
                    s
                ))
            }
            Hocon::String(s) => PrimitiveValue::String(s.to_string()),
            Hocon::Integer(n) => PrimitiveValue::Number(*n as f64),
            Hocon::Real(n) => PrimitiveValue::Number(*n as f64),
            Hocon::Boolean(b) => PrimitiveValue::Boolean(*b),
            _ => return Err(anyhow!("cannot convert {} to config::PrimitiveValue")),
        })
    }
}
