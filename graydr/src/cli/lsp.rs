// graydr/src/cli/lsp.rs
//
// `graydr lsp` — LSP server over stdio.
//
// CRITICAL: stdout is owned by the LSP transport. No println! anywhere in this
// file or any code path reachable from run_lsp(). Use eprintln! for debug
// output and client.log_message() for user-visible log lines.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp_server::jsonrpc::Result as LspResult;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

use crate::lint::{lint_module, LintDiagnostic, LintSeverity};
use crate::parser::error::ParseError;
use crate::parser::module::parse_module_file;
use crate::parser::template::parse_template_file;

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

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text.clone();
        let version = Some(params.text_document.version);
        // Update in-memory document state
        self.documents.write().await.insert(uri.clone(), text.clone());
        self.publish_file_diagnostics(uri, &text, version).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.last() {
            let uri = params.text_document.uri.clone();
            let text = change.text.clone();
            let version = Some(params.text_document.version);
            // Update in-memory document state
            self.documents.write().await.insert(uri.clone(), text.clone());
            self.publish_file_diagnostics(uri, &text, version).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        // Prefer text sent with save; fall back to in-memory state
        let text = if let Some(t) = params.text {
            t
        } else {
            self.documents
                .read()
                .await
                .get(&uri)
                .cloned()
                .unwrap_or_default()
        };
        if !text.is_empty() {
            self.publish_file_diagnostics(uri, &text, None).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        // Remove document from in-memory state; clear diagnostics
        let uri = params.text_document.uri.clone();
        self.documents.write().await.remove(&uri);
        self.client.publish_diagnostics(uri, vec![], None).await;
    }
}

impl Backend {
    /// Run parse + lint on the given file content and push publishDiagnostics.
    /// - `.gmod` files: parse errors + lint warnings/errors
    /// - `.gtpl` files: parse errors only (lint checks operate on ModuleDefinition)
    /// - Other extensions: no diagnostics emitted
    ///
    /// IMPORTANT: No println! here. Stdout belongs to the LSP transport.
    async fn publish_file_diagnostics(&self, uri: Uri, text: &str, version: Option<i32>) {
        let uri_str = uri.as_str();
        let ext = Path::new(uri_str).extension().and_then(|e| e.to_str()).unwrap_or("");

        let mut diagnostics: Vec<Diagnostic> = Vec::new();

        match ext {
            "gmod" => {
                // Parse errors
                match parse_module_file(text, uri_str) {
                    Ok(module) => {
                        // Parse succeeded — run lint checks on the in-memory AST
                        let lint_diags = lint_module(&module, text);
                        for d in &lint_diags {
                            diagnostics.push(lint_diag_to_lsp(d));
                        }
                    }
                    Err(e) => {
                        diagnostics.push(parse_error_to_lsp(&e));
                    }
                }
            }
            "gtpl" => {
                // Parse errors only
                if let Err(e) = parse_template_file(text, uri_str) {
                    diagnostics.push(parse_error_to_lsp(&e));
                }
            }
            _ => {
                // No diagnostics for unsupported file types
            }
        }

        self.client.publish_diagnostics(uri, diagnostics, version).await;
    }
}

fn lint_diag_to_lsp(d: &LintDiagnostic) -> Diagnostic {
    let line = d.line.saturating_sub(1) as u32; // 1-indexed -> 0-indexed
    let col = d.col.saturating_sub(1) as u32;
    Diagnostic {
        range: Range {
            start: Position { line, character: col },
            end: Position { line, character: col },
        },
        severity: Some(match d.severity {
            LintSeverity::Error => DiagnosticSeverity::ERROR,
            LintSeverity::Warning => DiagnosticSeverity::WARNING,
        }),
        source: Some("graydr".to_string()),
        message: format!("[{}] {}", d.check, d.message),
        ..Default::default()
    }
}

fn parse_error_to_lsp(e: &ParseError) -> Diagnostic {
    // Extract position from span-carrying variants; HclParse has no span.
    let (line, col) = match e {
        ParseError::MissingRequiredBlock { span, .. } => {
            (span.start_line.saturating_sub(1), span.start_col.saturating_sub(1))
        }
        ParseError::UnknownBlock { span, .. } => {
            (span.start_line.saturating_sub(1), span.start_col.saturating_sub(1))
        }
        ParseError::InvalidCaseLabel { span } => {
            (span.start_line.saturating_sub(1), span.start_col.saturating_sub(1))
        }
        ParseError::UnexpectedBlockType { span, .. } => {
            (span.start_line.saturating_sub(1), span.start_col.saturating_sub(1))
        }
        ParseError::MissingLabel { span, .. } => {
            (span.start_line.saturating_sub(1), span.start_col.saturating_sub(1))
        }
        ParseError::HclParse { .. } => (0, 0), // no span; point to line 0
    };

    Diagnostic {
        range: Range {
            start: Position { line: line as u32, character: col as u32 },
            end: Position { line: line as u32, character: col as u32 },
        },
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("graydr".to_string()),
        message: e.to_string(),
        ..Default::default()
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
