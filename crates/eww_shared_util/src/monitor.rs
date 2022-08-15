use std::{convert::Infallible, fmt, str};

use gdk::Monitor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitorIdentifier {
    Numeric(i32),
    Name(String),
}

impl MonitorIdentifier {
    pub fn get_monitor(&self, display: &gdk::Display) -> Option<Monitor> {
        display.monitor(match self {
            Self::Numeric(num) => *num,
            Self::Name(name) => {
                let mut idx = -1;
                for m in 0..display.n_monitors() {
                    if let Some(mon) = display.monitor(m) {
                        if let Some(model) = mon.model() {
                            if model == *name {
                                idx = m;
                                break;
                            }
                        }
                    }
                }
                idx
            }
        })
    }
}

impl fmt::Display for MonitorIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Numeric(n) => n.to_string(),
                Self::Name(n) => n.to_string(),
            }
        )
    }
}

impl str::FromStr for MonitorIdentifier {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<i32>() {
            Ok(n) => Ok(Self::Numeric(n)),
            Err(_) => Ok(Self::Name(s.to_owned())),
        }
    }
}
