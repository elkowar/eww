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

pub trait YuckFiles {
    fn load(&mut self, path: &str) -> Result<(Span, Vec<Ast>), FilesError>;
}

#[derive(Debug, Clone)]
pub struct FsYuckFiles {
    files: SimpleFiles<String, String>,
}

impl FsYuckFiles {
    pub fn new() -> Self {
        Self { files: SimpleFiles::new() }
    }
}

impl YuckFiles for FsYuckFiles {
    fn load(&mut self, path: &str) -> Result<(Span, Vec<Ast>), FilesError> {
        let path = PathBuf::from(path);

        let file_content = std::fs::read_to_string(&path)?;
        let file_id = self.files.add(path.display().to_string(), file_content.to_string());
        Ok(crate::parser::parse_toplevel(file_id, file_content)?)
    }
}

impl<'a> Files<'a> for FsYuckFiles {
    type FileId = usize;
    type Name = String;
    type Source = &'a str;

    fn name(&self, id: Self::FileId) -> Result<Self::Name, codespan_reporting::files::Error> {
        self.files.name(id)
    }

    fn source(&'a self, id: Self::FileId) -> Result<Self::Source, codespan_reporting::files::Error> {
        self.files.source(id)
    }

    fn line_index(&self, id: Self::FileId, byte_index: usize) -> Result<usize, codespan_reporting::files::Error> {
        self.files.line_index(id, byte_index)
    }

    fn line_range(
        &self,
        id: Self::FileId,
        line_index: usize,
    ) -> Result<std::ops::Range<usize>, codespan_reporting::files::Error> {
        self.files.line_range(id, line_index)
    }
}
