//! Rung 0 language server: lifecycle and document sync only.
//!
//! The server advertises exactly what exists — full text-document sync
//! — and nothing else: no diagnostics, no completion, no hover. Verse
//! intelligence arrives rung by rung behind its own gates (M1 parser
//! diagnostics, M2 digest navigation, M3 workspace index). Compilation
//! is never this server's claim: Build Verse Code in UEFN is the only
//! compile gate.
//!
//! State: the document store and the (empty until M2) symbol database
//! sit behind `RwLock`s; locks are never held across an `.await` other
//! than the lock acquisition itself, and handler work while holding a
//! lock is synchronous and bounded.

use tokio::sync::RwLock;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, InitializedParams, MessageType, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use verse_ls_db::SymbolDb;

use crate::document_store::{DocumentStore, StoreError};

/// The verse-ls language server (Rung 0).
pub struct VerseLanguageServer {
    client: Client,
    documents: RwLock<DocumentStore>,
    db: RwLock<SymbolDb>,
}

impl VerseLanguageServer {
    /// Create a server bound to the given LSP `client`.
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: RwLock::new(DocumentStore::new()),
            db: RwLock::new(SymbolDb::new()),
        }
    }

    /// Surface a document-store failure to the client as a bounded log
    /// message: URIs and counts only, never document content.
    async fn log_store_error(&self, error: &StoreError) {
        self.client
            .log_message(MessageType::WARNING, error.to_string())
            .await;
    }
}

impl LanguageServer for VerseLanguageServer {
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: String::from("verse-ls"),
                version: Some(String::from(env!("CARGO_PKG_VERSION"))),
            }),
            ..Default::default()
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(
                MessageType::INFO,
                "verse-ls initialized (rung 0: lifecycle and document sync only)",
            )
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        let documents = self.documents.read().await.document_count();
        let symbols = self.db.read().await.symbol_count();
        self.client
            .log_message(
                MessageType::INFO,
                format!("verse-ls shutting down: {documents} documents open, {symbols} symbols"),
            )
            .await;
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let document = params.text_document;
        let result = self.documents.write().await.open(
            document.uri.to_string(),
            document.version,
            document.text,
        );
        if let Err(error) = result {
            self.log_store_error(&error).await;
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // Full sync: the last change carries the document's whole text.
        let Some(change) = params.content_changes.into_iter().next_back() else {
            self.client
                .log_message(
                    MessageType::WARNING,
                    "didChange with no content changes; ignored",
                )
                .await;
            return;
        };
        let uri = params.text_document.uri.to_string();
        let result =
            self.documents
                .write()
                .await
                .change(&uri, params.text_document.version, change.text);
        if let Err(error) = result {
            self.log_store_error(&error).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        let result = self.documents.write().await.close(&uri);
        if let Err(error) = result {
            self.log_store_error(&error).await;
        }
    }
}

/// Run the server on stdin/stdout until the client exits the session.
///
/// The only transport in v1 is stdio, matching Diver's `verse_ls.lua`
/// spec (`verse-ls --stdio`).
pub async fn run_stdio() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(VerseLanguageServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
