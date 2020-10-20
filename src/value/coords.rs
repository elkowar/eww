use anyhow::*;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum NumWithUnit {
    Percent(i32),
    Pixels(i32),
}

impl fmt::Debug for NumWithUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl fmt::Display for NumWithUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NumWithUnit::Percent(x) => write!(f, "{}%", x),
            NumWithUnit::Pixels(x) => write!(f, "{}px", x),
        }
    }
}

impl FromStr for NumWithUnit {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        lazy_static::lazy_static! {
            static ref PATTERN: regex::Regex = regex::Regex::new("^(\\d+)(.*)$").unwrap();
        };

        let captures = PATTERN.captures(s).with_context(|| format!("could not parse '{}'", s))?;
        let value = captures.get(1).unwrap().as_str().parse::<i32>()?;
        let value = match captures.get(2).unwrap().as_str() {
            "px" | "" => NumWithUnit::Pixels(value),
            "%" => NumWithUnit::Percent(value),
            _ => bail!("couldn't parse {}, unit must be either px or %", s),
        };
        Ok(value)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct Coords {
    pub x: NumWithUnit,
    pub y: NumWithUnit,
}

impl FromStr for Coords {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (x, y) = s
            .split_once(|x: char| x.to_ascii_lowercase() == 'x')
            .ok_or_else(|| anyhow!("must be formatted like 200x500"))?;
        Coords::from_strs(x, y)
    }
}

impl fmt::Display for Coords {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}X{}", self.x, self.y)
    }
}

impl fmt::Debug for Coords {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CoordsWithUnits({}, {})", self.x, self.y)
    }
}

impl Coords {
    pub fn from_strs(x: &str, y: &str) -> Result<Coords> {
        Ok(Coords {
            x: x.parse()?,
            y: y.parse()?,
        })
    }
}
