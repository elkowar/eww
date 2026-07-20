use dashmap::{
    mapref::{multiple::RefMulti, one::Ref},
    DashMap,
};
use eww_shared_util::Span;
use tower_lsp::lsp_types::Url;
use yuck::{
    config::{
        file_provider::{FilesError, YuckFileProvider},
        Config,
    },
    error::DiagError,
    parser::ast::Ast,
};

#[derive(PartialEq, Clone)]
pub enum DocSource {
    String,
    File(Url),
}

#[derive(Clone)]
pub struct Document {
    name: String,
    text: String,
    source: DocSource,
    line_starts: Vec<usize>,
    source_len_bytes: usize,
}

#[derive(Clone)]
pub struct LspDocuments(pub DashMap<usize, Document>);

impl LspDocuments {
    pub fn new() -> Self {
        Self(DashMap::new())
    }

    pub fn get_file(&self, url: &Url) -> Result<RefMulti<'_, usize, Document>, codespan_reporting::files::Error> {
        self.0
            .iter()
            .find(|v| match &v.source {
                DocSource::File(f) => f == url,
                _ => false,
            })
            .ok_or(codespan_reporting::files::Error::FileMissing)
    }

    pub fn get_file_from_idx(&self, index: usize) -> Result<Ref<'_, usize, Document>, codespan_reporting::files::Error> {
        self.0.get(&index).ok_or(codespan_reporting::files::Error::FileMissing)
    }

    pub fn insert_file(&self, doc: Document) -> usize {
        let id = self.0.len();
        self.0.insert(id, doc).ok_or(codespan_reporting::files::Error::FileMissing);
        id
    }

    pub fn insert_string(&self, name: String, content: String) -> usize {
        let line_starts: Vec<_> = codespan_reporting::files::line_starts(&content).collect();
        let id = self.0.len();
        self.0
            .insert(id, Document { name, source: DocSource::String, line_starts, source_len_bytes: content.len(), text: content })
            .ok_or(codespan_reporting::files::Error::FileMissing);
        id
    }

    pub fn insert_url(&self, url: Url, content: String) -> usize {
        let line_starts: Vec<_> = codespan_reporting::files::line_starts(&content).collect();
        let doc = Document {
            name: url.to_string(),
            source: DocSource::File(url.clone()),
            line_starts,
            source_len_bytes: content.len(),
            text: content,
        };

        if let Ok(res) = self.get_file(&url) {
            let id = *res.key();
            self.0.insert(id, doc);
            return id;
        }

        let id = self.0.len();
        self.0.insert(id, doc).ok_or(codespan_reporting::files::Error::FileMissing);
        id
    }
}

impl YuckFileProvider for LspDocuments {
    fn load_yuck_file(&mut self, path: std::path::PathBuf) -> Result<(Span, Vec<Ast>), FilesError> {
        let file_content = std::fs::read_to_string(&path)?;
        let line_starts = codespan_reporting::files::line_starts(&file_content).collect();

        // TODO this is very bad and very stupid
        let source = match Url::from_file_path(&path) {
            Ok(v) => DocSource::File(v),
            Err(_) => DocSource::String,
        };

        let document = Document {
            name: path.display().to_string(),
            text: file_content.clone(),
            line_starts,
            source_len_bytes: file_content.len(),
            source,
        };
        let file_id = self.insert_file(document);
        Ok(yuck::parser::parse_toplevel(file_id, file_content)?)
    }

    fn load_yuck_str(&mut self, name: String, content: String) -> Result<(Span, Vec<Ast>), DiagError> {
        let file_id = self.insert_string(name, content.clone());
        yuck::parser::parse_toplevel(file_id, content)
    }

    fn unload(&mut self, id: usize) {
        self.0.remove(&id);
    }
}
