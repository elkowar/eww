use anyhow::*;
use grass;
use std::path::Path;

pub fn parse_scss_from_file<P: AsRef<Path>>(path: P) -> Result<String> {
    let scss_content = std::fs::read_to_string(path)?;
    grass::from_string(scss_content, &grass::Options::default())
        .map_err(|err| anyhow!("encountered SCSS parsing error: {:?}", err))
}
