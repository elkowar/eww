use std::{
    convert::Infallible,
    fmt,
    str::{self, FromStr},
};

use serde::{Deserialize, Serialize};
use simplexpr::dynval::{ConversionError, DynVal};

/// The type of the identifier used to select a monitor
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitorIdentifier {
    List(Vec<MonitorIdentifier>),
    Numeric(i32),
    Name(String),
    Primary,
}

impl MonitorIdentifier {
    pub fn from_dynval(val: &DynVal) -> Result<Self, ConversionError> {
        match val.as_json_array() {
            Ok(arr) => Ok(MonitorIdentifier::List(
                arr.iter().map(|x| MonitorIdentifier::from_dynval(&x.into())).collect::<Result<_, _>>()?,
            )),
            Err(_) => match val.as_i32() {
                Ok(x) => Ok(MonitorIdentifier::Numeric(x)),
                Err(_) => Ok(MonitorIdentifier::from_str(&val.as_string().unwrap()).unwrap()),
            },
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Self::Numeric(_))
    }
}

impl From<&MonitorIdentifier> for DynVal {
    fn from(val: &MonitorIdentifier) -> Self {
        match val {
            MonitorIdentifier::List(l) => l.iter().map(|x| x.into()).collect::<Vec<_>>().into(),
            MonitorIdentifier::Numeric(n) => DynVal::from(*n),
            MonitorIdentifier::Name(n) => DynVal::from(n.clone()),
            MonitorIdentifier::Primary => DynVal::from("<primary>"),
        }
    }
}

impl fmt::Display for MonitorIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::List(l) => write!(f, "[{}]", l.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ")),
            Self::Numeric(n) => write!(f, "{}", n),
            Self::Name(n) => write!(f, "{}", n),
            Self::Primary => write!(f, "<primary>"),
        }
    }
}

impl str::FromStr for MonitorIdentifier {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<i32>() {
            Ok(n) => Ok(Self::Numeric(n)),
            Err(_) => {
                if &s.to_lowercase() == "<primary>" {
                    Ok(Self::Primary)
                } else {
                    Ok(Self::Name(s.to_owned()))
                }
            }
        }
    }
}
