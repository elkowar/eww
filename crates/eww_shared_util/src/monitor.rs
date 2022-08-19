use std::{convert::Infallible, fmt, str};

use gdk::Monitor;
use serde::{Deserialize, Serialize};

/// The type of the identifier used to select a monitor
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitorIdentifier {
    Numeric(i32),
    Name(String),
}

impl MonitorIdentifier {
    /// Returns the [Monitor][gdk::Monitor] structure corresponding to the identifer
    // wayland argument is a hack so that this package
    // doesn't have to include features for x11 and wayland
    // remove when it becomes possible/easy to get output names in wayland
    pub fn get_monitor(&self, display: &gdk::Display, wayland: bool) -> Option<Monitor> {
        display.monitor(match self {
            Self::Numeric(num) => *num,
            Self::Name(name) => {
                if wayland {
                    return None;
                } else {
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
            }
        })
    }

    // only needed because we only support numeric identifiers for wayland for now
    pub fn is_numeric(&self) -> bool {
        match self {
            Self::Numeric(_) => true,
            _ => false,
        }
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
