use std::{convert::Infallible, fmt, str};

use serde::{Deserialize, Serialize};

/// The type of the identifier used to select a monitor
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitorIdentifier {
    List(Vec<MonitorIdentifier>),
    Numeric(i32),
    Name(String),
    Primary,
}

impl MonitorIdentifier {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Self::Numeric(_))
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
