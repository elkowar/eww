use std::path::Path;

use anyhow::{anyhow, Context};

use crate::{error_handling_ctx, util::replace_env_var_references};

/// read an scss file, replace all environment variable references within it and
/// then parse it into css.
/// Also adds the CSS to the [`crate::file_database::FileDatabase`]
pub fn parse_scss_from_file(path: &Path) -> anyhow::Result<(usize, String)> {
    let config_dir = path.parent().context("Given SCSS file has no parent directory?!")?;
    let scss_file_content =
        std::fs::read_to_string(path).with_context(|| format!("Given SCSS-file doesn't exist! {}", path.display()))?;
    let file_content = replace_env_var_references(scss_file_content);
    let grass_config = grass::Options::default().load_path(config_dir);
    let css = grass::from_string(file_content, &grass_config).map_err(|err| anyhow!("SCSS parsing error: {}", err))?;

    let mut file_db = error_handling_ctx::FILE_DATABASE.write().unwrap();
    let file_id = file_db.insert_string(path.display().to_string(), css.clone())?;
    Ok((file_id, css))
}
