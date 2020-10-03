use anyhow::*;
use extend::ext;
use grass;
use itertools::Itertools;
use std::path::Path;

pub fn parse_scss_from_file<P: AsRef<Path>>(path: P) -> Result<String> {
    let scss_content = std::fs::read_to_string(path)?;
    grass::from_string(scss_content, &grass::Options::default())
        .map_err(|err| anyhow!("encountered SCSS parsing error: {:?}", err))
}

#[ext(pub, name = StringExt)]
impl<T: AsRef<str>> T {
    /// check if the string is empty after removing all linebreaks and trimming whitespace
    fn is_blank(self) -> bool {
        self.as_ref().replace('\n', "").trim().is_empty()
    }

    /// trim all lines in a string
    fn trim_lines(self) -> String {
        self.as_ref().lines().map(|line| line.trim()).join("\n")
    }
}
