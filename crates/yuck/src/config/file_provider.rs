use std::{collections::HashMap, path::PathBuf};

use codespan_reporting::files::{Files, SimpleFile, SimpleFiles};
use eww_shared_util::Span;

use crate::{
    error::{AstError, AstResult},
    parser::ast::Ast,
};

#[derive(thiserror::Error, Debug)]
pub enum FilesError {
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    AstError(#[from] AstError),
}

#[derive(Clone, Debug)]
pub enum YuckSource {
    File(std::path::PathBuf),
    Literal(String),
}

impl YuckSource {
    pub fn read_content(&self) -> std::io::Result<String> {
        match self {
            YuckSource::File(path) => Ok(std::fs::read_to_string(path)?),
            YuckSource::Literal(x) => Ok(x.to_string()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct YuckFile {
    name: String,
    line_starts: Vec<usize>,
    source: YuckSource,
    source_len_bytes: usize,
}

impl YuckFile {
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
pub struct YuckFiles {
    files: HashMap<usize, YuckFile>,
    latest_id: usize,
}

impl YuckFiles {
    pub fn new() -> Self {
        Self::default()
    }
}

impl YuckFiles {
    pub fn get_file(&self, id: usize) -> Result<&YuckFile, codespan_reporting::files::Error> {
        self.files.get(&id).ok_or(codespan_reporting::files::Error::FileMissing)
    }

    fn insert_file(&mut self, file: YuckFile) -> usize {
        let file_id = self.latest_id;
        self.files.insert(file_id, file);
        self.latest_id += 1;
        file_id
    }

    pub fn load_file(&mut self, path: std::path::PathBuf) -> Result<(Span, Vec<Ast>), FilesError> {
        let file_content = std::fs::read_to_string(&path)?;
        let line_starts = codespan_reporting::files::line_starts(&file_content).collect();
        let yuck_file = YuckFile {
            name: path.display().to_string(),
            line_starts,
            source_len_bytes: file_content.len(),
            source: YuckSource::File(path),
        };
        let file_id = self.insert_file(yuck_file);
        Ok(crate::parser::parse_toplevel(file_id, file_content)?)
    }

    pub fn load_str(&mut self, name: String, content: String) -> Result<(Span, Vec<Ast>), AstError> {
        let line_starts = codespan_reporting::files::line_starts(&content).collect();
        let yuck_file =
            YuckFile { name, line_starts, source_len_bytes: content.len(), source: YuckSource::Literal(content.to_string()) };
        let file_id = self.insert_file(yuck_file);
        crate::parser::parse_toplevel(file_id, content)
    }

    pub fn unload(&mut self, id: usize) {
        self.files.remove(&id);
    }
}

impl<'a> Files<'a> for YuckFiles {
    type FileId = usize;
    type Name = &'a str;
    type Source = String;

    fn name(&'a self, id: Self::FileId) -> Result<Self::Name, codespan_reporting::files::Error> {
        Ok(&self.get_file(id)?.name)
    }

    fn source(&'a self, id: Self::FileId) -> Result<Self::Source, codespan_reporting::files::Error> {
        self.get_file(id)?.source.read_content().map_err(codespan_reporting::files::Error::Io)
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
