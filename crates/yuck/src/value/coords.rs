use derive_more::*;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::{fmt, str::FromStr};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to parse \"{0}\" as a length value")]
    NumParseFailed(String),
    #[error("Invalid unit \"{0}\", must be either % or px")]
    InvalidUnit(String),
    #[error("Invalid format. Coordinates must be formated like 200x100")]
    MalformedCoords,
}

#[derive(Clone, Copy, PartialEq, Deserialize, Serialize, Display, DebugCustom, SmartDefault)]
pub enum NumWithUnit {
    #[display(fmt = "{}%", .0)]
    #[debug(fmt = "{}%", .0)]
    Percent(f32),
    #[display(fmt = "{}px", .0)]
    #[debug(fmt = "{}px", .0)]
    #[default]
    Pixels(i32),
}

impl NumWithUnit {
    pub fn pixels_relative_to(&self, max: i32) -> i32 {
        match *self {
            NumWithUnit::Percent(n) => ((max as f64 / 100.0) * n as f64) as i32,
            NumWithUnit::Pixels(n) => n,
        }
    }

    pub fn perc_relative_to(&self, max: i32) -> f32 {
        match *self {
            NumWithUnit::Percent(n) => n,
            NumWithUnit::Pixels(n) => ((n as f64 / max as f64) * 100.0) as f32,
        }
    }
}

impl FromStr for NumWithUnit {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        static PATTERN: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new("^(-?\\d+(?:.\\d+)?)(.*)$").unwrap());

        let captures = PATTERN.captures(s).ok_or_else(|| Error::NumParseFailed(s.to_string()))?;
        let value = captures.get(1).unwrap().as_str().parse::<f32>().map_err(|_| Error::NumParseFailed(s.to_string()))?;
        match captures.get(2).unwrap().as_str() {
            "px" | "" => Ok(NumWithUnit::Pixels(value.floor() as i32)),
            "%" => Ok(NumWithUnit::Percent(value)),
            unit => Err(Error::InvalidUnit(unit.to_string())),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Deserialize, Serialize, Display, Default)]
#[display(fmt = "{}*{}", x, y)]
pub struct Coords {
    pub x: NumWithUnit,
    pub y: NumWithUnit,
}

impl FromStr for Coords {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (x, y) = s
            .split_once(|x: char| x.to_ascii_lowercase() == 'x' || x.to_ascii_lowercase() == '*')
            .ok_or(Error::MalformedCoords)?;
        Coords::from_strs(x, y)
    }
}

impl fmt::Debug for Coords {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CoordsWithUnits({}, {})", self.x, self.y)
    }
}

impl Coords {
    pub fn from_pixels((x, y): (i32, i32)) -> Self {
        Coords { x: NumWithUnit::Pixels(x), y: NumWithUnit::Pixels(y) }
    }

    /// parse a string for x and a string for y into a [`Coords`] object.
    pub fn from_strs(x: &str, y: &str) -> Result<Coords, Error> {
        Ok(Coords { x: x.parse()?, y: y.parse()? })
    }

    /// resolve the possibly relative coordinates relative to a given containers size
    pub fn relative_to(&self, width: i32, height: i32) -> (i32, i32) {
        (self.x.pixels_relative_to(width), self.y.pixels_relative_to(height))
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
        assert_eq!(NumWithUnit::Percent(55.0), NumWithUnit::from_str("55%").unwrap());
        assert_eq!(NumWithUnit::Percent(55.5), NumWithUnit::from_str("55.5%").unwrap());
        assert!(NumWithUnit::from_str("55pp").is_err());
    }

    #[test]
    fn test_parse_coords() {
        assert_eq!(Coords { x: NumWithUnit::Pixels(50), y: NumWithUnit::Pixels(60) }, Coords::from_str("50x60").unwrap());
        assert!(Coords::from_str("5060").is_err());
    }
}
