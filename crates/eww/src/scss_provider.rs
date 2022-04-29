use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Result};
use codespan_reporting::files::Files;
use eww_shared_util::Span;

pub struct Css(pub String);

#[derive(Clone, Debug)]
pub struct ScssFile {
    name: String,
    line_starts: Vec<usize>,
    source: PathBuf,
    source_len_bytes: usize,
}

impl ScssFile {
    /// Return the starting byte index of the line with the specified line index.
    /// Convenience method that already generates errors if necessary.
    fn line_start(&self, line_index: usize) -> Result<usize, codespan_reporting::files::Error> {
        use std::cmp::Ordering;

        match line_index.cmp(&self.line_starts.len()) {
            Ordering::Less => Ok(self.line_starts.get(line_index).cloned().expect("failed despite previous check")),
            Ordering::Equal => Ok(self.source_len_bytes),
            Ordering::Greater => {
                Err(codespan_reporting::files::Error::LineTooLarge { given: line_index, max: self.line_starts.len() - 1 })
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScssFiles {
    config_dir: PathBuf,
    files: HashMap<usize, ScssFile>,
    latest_id: usize,
}

impl ScssFiles {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ScssFiles {
    pub fn get_file(&self, id: usize) -> Result<&ScssFile, codespan_reporting::files::Error> {
        self.files.get(&id).ok_or(codespan_reporting::files::Error::FileMissing)
    }

    fn insert_file(&mut self, file: ScssFile) -> usize {
        let file_id = self.latest_id;
        self.files.insert(file_id, file);
        self.latest_id += 1;
        file_id
    }

    pub fn load_file(&mut self, path: std::path::PathBuf) -> Result<(usize, Css)> {
        // TODO implement env var preprocessing
        let file_content = std::fs::read_to_string(&path)?;
        let line_starts = codespan_reporting::files::line_starts(&file_content).collect();
        let scss_file =
            ScssFile { name: path.display().to_string(), line_starts, source_len_bytes: file_content.len(), source: path };
        let file_id = self.insert_file(scss_file);

        let grass_config = grass::Options::default().load_path(&self.config_dir);

        match grass::from_string(file_content, &grass_config) {
            Ok(css) => Ok((file_id, Css(css))),
            // TODO bad error handling
            Err(err) => Err(anyhow!(err.to_string())),
        }
    }

    pub fn unload(&mut self, id: usize) {
        self.files.remove(&id);
    }
}

impl<'a> Files<'a> for ScssFiles {
    type FileId = usize;
    type Name = &'a str;
    type Source = String;

    fn name(&'a self, id: Self::FileId) -> Result<Self::Name, codespan_reporting::files::Error> {
        Ok(&self.get_file(id)?.name)
    }

    fn source(&'a self, id: Self::FileId) -> Result<Self::Source, codespan_reporting::files::Error> {
        std::fs::read_to_string(&self.get_file(id)?.source).map_err(codespan_reporting::files::Error::Io)
    }

    fn line_index(&self, id: Self::FileId, byte_index: usize) -> Result<usize, codespan_reporting::files::Error> {
        Ok(self.get_file(id)?.line_starts.binary_search(&byte_index).unwrap_or_else(|next_line| next_line - 1))
    }

    fn line_range(
        &self,
        id: Self::FileId,
        line_index: usize,
    ) -> Result<std::ops::Range<usize>, codespan_reporting::files::Error> {
        let file = self.get_file(id)?;
        let line_start = file.line_start(line_index)?;
        let next_line_start = file.line_start(line_index + 1)?;
        Ok(line_start..next_line_start)
    }
}



// read an scss file, replace all environment variable references within it and
// then parse it into css.
//pub fn parse_scss_from_file(path: &Path) -> Result<String> {
    //let config_dir = path.parent().context("Given SCSS file has no parent directory?!")?;
    //let scss_file_content =
        //std::fs::read_to_string(path).with_context(|| format!("Given SCSS File Doesnt Exist! {}", path.display()))?;
    //let file_content = replace_env_var_references(scss_file_content);
    //let grass_config = grass::Options::default().load_path(config_dir);
    //grass::from_string(file_content, &grass_config).map_err(|err| anyhow!("Encountered SCSS parsing error: {:?}", err))
//}
