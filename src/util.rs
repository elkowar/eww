use anyhow::*;
use extend::ext;
use grass;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{fmt, path::Path};

#[macro_export]
macro_rules! impl_try_from {
    ($typ:ty {
        $(
            for $for:ty => |$arg:ident| $code:expr
        );*;
    }) => {
        $(impl TryFrom<$typ> for $for {
            type Error = anyhow::Error;

            fn try_from($arg: $typ) -> Result<Self> {
                $code
            }
        })*
    };
}

/// read an scss file, replace all environment variable references within it and
/// then parse it into css.
pub fn parse_scss_from_file<P: AsRef<Path>>(path: P) -> Result<String> {
    let scss_content = replace_env_var_references(std::fs::read_to_string(path)?);
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

/// Replace all env-var references of the format `"something $foo"` in a string
/// by the actual env-variables. If the env-var isn't found, will replace the
/// reference with an empty string.
pub fn replace_env_var_references(input: String) -> String {
    lazy_static::lazy_static! {
        static ref ENV_VAR_PATTERN: regex::Regex = regex::Regex::new(r"\$([^\s]*)").unwrap();
    }
    ENV_VAR_PATTERN
        .replace_all(&input, |var_name: &regex::Captures| {
            std::env::var(var_name.get(1).unwrap().as_str()).unwrap_or_default()
        })
        .into_owned()
}

/// If the given result is `Err`, prints out the error value using `{:?}`
pub fn print_result_err<T, E: std::fmt::Debug>(context: &str, result: &std::result::Result<T, E>) {
    if let Err(err) = result {
        eprintln!("Error {}: {:?}", context, err);
    }
}
