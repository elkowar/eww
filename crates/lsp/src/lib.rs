use tower_lsp::jsonrpc::Result;
use tower_lsp::{lsp_types::*, Client, LanguageServer};

pub struct Backend {
    pub client: Client,
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
        todo!()
    }
}
