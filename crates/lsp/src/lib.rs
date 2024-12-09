use std::sync::{Arc, Mutex};

use dashmap::DashMap;
use documents::LspDocuments;
use tower_lsp::jsonrpc::Result;
use tower_lsp::{lsp_types::*, Client, LanguageServer};
use yuck::config::Config;

mod documents;

use yuck::error::DiagError;
use yuck::parser::parse_toplevel;

pub type ArcM<T> = Arc<Mutex<T>>;

pub struct Backend {
    pub client: Client,
    documents: LspDocuments,
    config: ArcM<Option<Config>>,
    errors: ArcM<Vec<DiagError>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities { document_highlight_provider: Some(OneOf::Left(true)), ..Default::default() },
            server_info: None,
        })
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "server initialized!").await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.change_document(TextDocumentItem {
            uri: params.text_document.uri,
            version: Some(params.text_document.version),
            text: params.text_document.text,
        })
        .await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        self.change_document(TextDocumentItem {
            uri: params.text_document.uri,
            version: Some(params.text_document.version),
            text: params.content_changes.swap_remove(0).text,
        })
        .await;
    }
}

pub struct TextDocumentItem {
    uri: Url,
    text: String,
    version: Option<i32>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self { client, documents: LspDocuments::new(), config: Default::default(), errors: Default::default() }
    }

    async fn change_document(&self, doc: TextDocumentItem) {
        let file_id = self.documents.insert_url(doc.uri, doc.text.clone());
        let mut new_errors = Vec::new();

        let mut errors = self.errors.lock().unwrap();
        let mut config = self.config.lock().unwrap();

        let mut new_documents = self.documents.clone();
        *config = match parse_toplevel(file_id, doc.text).map(|(_, asts)| Config::generate(&mut new_documents, asts)) {
            Ok(Ok(v)) => Some(v),
            Err(e) | Ok(Err(e)) => {
                new_errors.push(e);
                None
            }
        };
        *errors = new_errors;

        // TODO this is extremely stupid, but I need to do this so I don't mutate self.
        // A solution that doesn't need to mutate `self.documents` would be ideal, but
        // `Config::generate` needs to mutate the documents. Unsure how best to handle this
        self.documents.0.clear();
        new_documents.0.into_iter().for_each(|v| {
            self.documents.0.insert(v.0, v.1);
        });
    }
}
