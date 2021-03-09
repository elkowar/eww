use anyhow::*;
use derive_more::*;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::{fmt, str::FromStr};

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Display, DebugCustom, SmartDefault)]
pub enum NumWithUnit {
    #[display(fmt = "{}%", .0)]
    #[debug(fmt = "{}%", .0)]
    Percent(i32),
    #[display(fmt = "{}px", .0)]
    #[debug(fmt = "{}px", .0)]
    #[default]
    Pixels(i32),
}

impl NumWithUnit {
    pub fn relative_to(&self, max: i32) -> i32 {
        match *self {
            NumWithUnit::Percent(n) => ((max as f64 / 100.0) * n as f64) as i32,
            NumWithUnit::Pixels(n) => n,
        }
    }
}

impl FromStr for NumWithUnit {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        lazy_static::lazy_static! {
            static ref PATTERN: regex::Regex = regex::Regex::new("^(-?\\d+)(.*)$").unwrap();
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

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Display, Default)]
#[display(fmt = "{}*{}", x, y)]
pub struct Coords {
    pub x: NumWithUnit,
    pub y: NumWithUnit,
}

impl FromStr for Coords {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (x, y) = s
            .split_once(|x: char| x.to_ascii_lowercase() == 'x' || x.to_ascii_lowercase() == '*')
            .ok_or_else(|| anyhow!("must be formatted like 200x500"))?;
        Coords::from_strs(x, y)
    }
}

impl fmt::Debug for Coords {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CoordsWithUnits({}, {})", self.x, self.y)
    }
}

impl Coords {
    pub fn from_pixels(x: i32, y: i32) -> Self {
        Coords { x: NumWithUnit::Pixels(x), y: NumWithUnit::Pixels(y) }
    }

    /// parse a string for x and a string for y into a [`Coords`] object.
    pub fn from_strs(x: &str, y: &str) -> Result<Coords> {
        Ok(Coords {
            x: x.parse().with_context(|| format!("Failed to parse '{}'", x))?,
            y: y.parse().with_context(|| format!("Failed to parse '{}'", y))?,
        })
    }

    /// resolve the possibly relative coordinates relative to a given containers size
    pub fn relative_to(&self, width: i32, height: i32) -> (i32, i32) {
        (self.x.relative_to(width), self.y.relative_to(height))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_num_with_unit() {
        assert_eq!(NumWithUnit::Pixels(55), NumWithUnit::from_str("55").unwrap());
        assert_eq!(NumWithUnit::Pixels(55), NumWithUnit::from_str("55px").unwrap());
        assert_eq!(NumWithUnit::Percent(55), NumWithUnit::from_str("55%").unwrap());
        assert!(NumWithUnit::from_str("55pp").is_err());
    }

    #[test]
    fn test_parse_coords() {
        assert_eq!(Coords { x: NumWithUnit::Pixels(50), y: NumWithUnit::Pixels(60) }, Coords::from_str("50x60").unwrap());
        assert!(Coords::from_str("5060").is_err());
    }
}
