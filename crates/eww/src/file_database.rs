use std::collections::HashMap;

use codespan_reporting::files::Files;
use eww_shared_util::Span;
use yuck::{
    config::file_provider::{FilesError, YuckFileProvider},
    error::DiagError,
    parser::ast::Ast,
};

#[derive(Debug, Clone, Default)]
pub struct FileDatabase {
    files: HashMap<usize, CodeFile>,
    latest_id: usize,
}

impl FileDatabase {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_file(&self, id: usize) -> Result<&CodeFile, codespan_reporting::files::Error> {
        self.files.get(&id).ok_or(codespan_reporting::files::Error::FileMissing)
    }

    fn insert_code_file(&mut self, file: CodeFile) -> usize {
        let file_id = self.latest_id;
        self.files.insert(file_id, file);
        self.latest_id += 1;
        file_id
    }

    pub fn insert_string(&mut self, name: String, content: String) -> Result<usize, DiagError> {
        let line_starts = codespan_reporting::files::line_starts(&content).collect();
        let code_file = CodeFile { name, line_starts, source_len_bytes: content.len(), source: CodeSource::Literal(content) };
        let file_id = self.insert_code_file(code_file);
        Ok(file_id)
    }
}

impl YuckFileProvider for FileDatabase {
    fn load_yuck_file(&mut self, path: std::path::PathBuf) -> Result<(Span, Vec<Ast>), FilesError> {
        let file_content = std::fs::read_to_string(&path)?;
        let line_starts = codespan_reporting::files::line_starts(&file_content).collect();
        let code_file = CodeFile {
            name: path.display().to_string(),
            line_starts,
            source_len_bytes: file_content.len(),
            source: CodeSource::File(path),
        };
        let file_id = self.insert_code_file(code_file);
        Ok(yuck::parser::parse_toplevel(file_id, file_content)?)
    }

    fn load_yuck_str(&mut self, name: String, content: String) -> Result<(Span, Vec<Ast>), DiagError> {
        let file_id = self.insert_string(name, content.clone())?;
        yuck::parser::parse_toplevel(file_id, content)
    }

    fn unload(&mut self, id: usize) {
        self.files.remove(&id);
    }
}

impl<'a> Files<'a> for FileDatabase {
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

#[derive(Clone, Debug)]
struct CodeFile {
    name: String,
    line_starts: Vec<usize>,
    source: CodeSource,
    source_len_bytes: usize,
}

impl CodeFile {
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

#[derive(Clone, Debug)]
enum CodeSource {
    File(std::path::PathBuf),
    Literal(String),
}

impl CodeSource {
    fn read_content(&self) -> std::io::Result<String> {
        match self {
            CodeSource::File(path) => Ok(std::fs::read_to_string(path)?),
            CodeSource::Literal(x) => Ok(x.to_string()),
        }
    }
}
