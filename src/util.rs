use anyhow::*;
use extend::ext;
use grass;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{fmt, path::Path};

pub fn parse_scss_from_file<P: AsRef<Path>>(path: P) -> Result<String> {
    let scss_content = std::fs::read_to_string(path)?;
    grass::from_string(scss_content, &grass::Options::default())
        .map_err(|err| anyhow!("encountered SCSS parsing error: {:?}", err))
}

#[ext(pub, name = StringExt)]
impl<T: AsRef<str>> T {
    /// check if the string is empty after removing all linebreaks and trimming
    /// whitespace
    fn is_blank(self) -> bool {
        self.as_ref().replace('\n', "").trim().is_empty()
    }

    /// trim all lines in a string
    fn trim_lines(self) -> String {
        self.as_ref().lines().map(|line| line.trim()).join("\n")
    }
}

pub fn parse_duration(s: &str) -> Result<std::time::Duration> {
    use std::time::Duration;
    if s.ends_with("ms") {
        Ok(Duration::from_millis(s.trim_end_matches("ms").parse()?))
    } else if s.ends_with("s") {
        Ok(Duration::from_secs(s.trim_end_matches("s").parse()?))
    } else if s.ends_with("m") {
        Ok(Duration::from_secs(s.trim_end_matches("m").parse::<u64>()? * 60))
    } else if s.ends_with("h") {
        Ok(Duration::from_secs(s.trim_end_matches("h").parse::<u64>()? * 60 * 60))
    } else {
        Err(anyhow!("unrecognized time format: {}", s))
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub struct Coords(pub i32, pub i32);

impl fmt::Display for Coords {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}x{}", self.0, self.1)
    }
}

impl std::str::FromStr for Coords {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let (x, y) = s.split_once('x').ok_or_else(|| anyhow!("must be formatted like 200x500"))?;
        Ok(Coords(x.parse()?, y.parse()?))
    }
}

impl From<(i32, i32)> for Coords {
    fn from((x, y): (i32, i32)) -> Self {
        Coords(x, y)
    }
}
