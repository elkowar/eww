use std::path::Path;

use anyhow::{anyhow, Context};

use crate::util::replace_env_var_references;

/// read an scss file, replace all environment variable references within it and
/// then parse it into css.
pub fn parse_scss_from_file(path: &Path) -> anyhow::Result<String> {
    let config_dir = path.parent().context("Given SCSS file has no parent directory?!")?;
    let scss_file_content =
        std::fs::read_to_string(path).with_context(|| format!("Given SCSS-file doesn't exist! {}", path.display()))?;
    let file_content = replace_env_var_references(scss_file_content);
    let grass_config = grass::Options::default().load_path(config_dir);
    grass::from_string(file_content, &grass_config).map_err(|err| anyhow!("Encountered SCSS parsing error: {}", err))
}
