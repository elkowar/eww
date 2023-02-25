use std::path::Path;

use anyhow::{anyhow, Context};

use crate::{error_handling_ctx, util::replace_env_var_references};

/// read an (s)css file, replace all environment variable references within it and
/// then parse it into css.
/// Also adds the CSS to the [`crate::file_database::FileDatabase`]
pub fn parse_scss_from_config(path: &Path) -> anyhow::Result<(usize, String)> {
    let css_file = path.join("eww.css");
    let scss_file = path.join("eww.scss");
    if css_file.exists() && scss_file.exists() {
        return Err(anyhow!("Encountered both an SCSS and CSS file. Only one of these may exist at a time"));
    }

    let (s_css_path, css) = if css_file.exists() {
        let css_file_content = std::fs::read_to_string(&css_file)
            .with_context(|| format!("Given CSS file doesn't exist: {}", css_file.display()))?;
        let css = replace_env_var_references(css_file_content);
        (css_file, css)
    } else {
        let scss_file_content =
            std::fs::read_to_string(&scss_file).with_context(|| format!("Given SCSS file doesn't exist! {}", path.display()))?;
        let file_content = replace_env_var_references(scss_file_content);
        let grass_config = grass::Options::default().load_path(path);
        let css = grass::from_string(file_content, &grass_config).map_err(|err| anyhow!("SCSS parsing error: {}", err))?;
        (scss_file, css)
    };

    let mut file_db = error_handling_ctx::FILE_DATABASE.write().unwrap();
    let file_id = file_db.insert_string(s_css_path.display().to_string(), css.clone())?;
    Ok((file_id, css))
}
