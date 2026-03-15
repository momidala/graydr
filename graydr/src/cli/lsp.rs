// graydr/src/cli/lsp.rs
//
// `graydr lsp` — LSP server over stdio.
//
// CRITICAL: stdout is owned by the LSP transport. No println! anywhere in this
// file or any code path reachable from run_lsp(). Use eprintln! for debug
// output and client.log_message() for user-visible log lines.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp_server::jsonrpc::Result as LspResult;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
struct Backend {
    client: Client,
    /// In-memory document state: URI -> latest text content.
    /// Populated by did_open/did_change, used by did_save as fallback.
    documents: Arc<RwLock<HashMap<Uri, String>>>,
}

impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "graydr".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            offset_encoding: None,
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        // Server is ready. Log to stderr to avoid stdout pollution.
        eprintln!("graydr lsp ready");
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }
}

pub fn run_lsp() {
    tokio::runtime::Runtime::new()
        .expect("failed to create tokio runtime for LSP")
        .block_on(async {
            let (service, socket) = LspService::new(|client| Backend {
                client,
                documents: Arc::new(RwLock::new(HashMap::new())),
            });
            Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
                .serve(service)
                .await;
        });
}
