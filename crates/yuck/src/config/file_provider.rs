use std::{collections::HashMap, path::PathBuf};

use codespan_reporting::files::{Files, SimpleFile, SimpleFiles};
use eww_shared_util::Span;

use crate::{
    error::{DiagError, DiagResult},
    parser::ast::Ast,
};

#[derive(thiserror::Error, Debug)]
pub enum FilesError {
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    DiagError(#[from] DiagError),
}

pub trait YuckFileProvider {
    fn load_yuck_file(&mut self, path: std::path::PathBuf) -> Result<(Span, Vec<Ast>), FilesError>;
    fn load_yuck_str(&mut self, name: String, content: String) -> Result<(Span, Vec<Ast>), DiagError>;
    fn unload(&mut self, id: usize);
}
