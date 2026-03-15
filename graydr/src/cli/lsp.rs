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
    /// Workspace root URI from initialize params — used for module file resolution.
    /// Stored as a plain String (the URI's string form). None if client did not provide.
    root_uri: Arc<RwLock<Option<String>>>,
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        // Store workspace root for module resolution in completion/definition handlers
        if let Some(uri) = params.root_uri {
            *self.root_uri.write().await = Some(uri.to_string());
        } else if let Some(folders) = params.workspace_folders {
            if let Some(first) = folders.into_iter().next() {
                *self.root_uri.write().await = Some(first.uri.to_string());
            }
        }

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
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: None,
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
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

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = &params.text_document_position.position;

        // Only provide completions in .gtpl files (inputs block context)
        let uri_str = uri.as_str();
        if !uri_str.ends_with(".gtpl") {
            return Ok(None);
        }

        // Get current document text
        let text = {
            let docs = self.documents.read().await;
            match docs.get(uri) {
                Some(t) => t.clone(),
                None => return Ok(None),
            }
        };

        // Parse the template AST
        let template = match parse_template_file(&text, uri_str) {
            Ok(t) => t,
            Err(_) => return Ok(None), // Incomplete document; no completions
        };

        // Find which resource's block contains the cursor position
        // Span uses 1-indexed lines/cols; LSP position is 0-indexed.
        let cursor_line = position.line as u32 + 1; // 0-indexed -> 1-indexed

        let mut module_ref: Option<String> = None;
        for resource in &template.resources {
            let res_span = &resource.span;
            // Check if cursor is inside this resource's span
            if cursor_line < res_span.start_line || cursor_line > res_span.end_line {
                continue;
            }
            // Cursor is inside this resource block — use its module_ref for completions
            module_ref = Some(resource.value.module_ref.value.clone());
            break;
        }

        let module_name = match module_ref {
            Some(m) => m,
            None => return Ok(None),
        };

        // Find the .gmod file: try workspace root first, then document directory as fallback.
        let root = {
            let r = self.root_uri.read().await;
            r.clone()
        };
        let gmod_path = if root.is_some() {
            find_gmod_file(&root, &module_name)?
        } else {
            // Derive search root from the document's own directory
            let doc_dir = uri_to_parent_dir(uri_str);
            find_gmod_file(&doc_dir, &module_name)?
        };
        let gmod_path = match gmod_path {
            Some(p) => p,
            None => return Ok(None),
        };

        // Parse the module to get its interface inputs
        let gmod_text = match std::fs::read_to_string(&gmod_path) {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };
        let module = match parse_module_file(&gmod_text, &gmod_path.to_string_lossy()) {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };

        // Build completion items from interface inputs
        let items: Vec<CompletionItem> = module
            .interface
            .value
            .inputs
            .iter()
            .map(|inp| {
                let name = inp.value.name.value.clone();
                let detail = if inp.value.required {
                    "required".to_string()
                } else {
                    "optional".to_string()
                };
                CompletionItem {
                    label: name,
                    kind: Some(CompletionItemKind::FIELD),
                    detail: Some(detail),
                    ..Default::default()
                }
            })
            .collect();

        if items.is_empty() {
            return Ok(None);
        }
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = &params.text_document_position_params.position;

        // Get current document text
        let text = {
            let docs = self.documents.read().await;
            match docs.get(uri) {
                Some(t) => t.clone(),
                None => return Ok(None),
            }
        };

        // Extract the word at the cursor position
        let word = extract_word_at_position(
            &text,
            position.line as usize,
            position.character as usize,
        );
        let word = match word {
            Some(w) => w,
            None => return Ok(None),
        };

        // Look up in governance metadata table
        let content = match governance_hover(&word) {
            Some(c) => c,
            None => return Ok(None),
        };

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: content.to_string(),
            }),
            range: None,
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = &params.text_document_position_params.position;
        let uri_str = uri.as_str();

        // Only handle .gtpl files (module references live in templates)
        if !uri_str.ends_with(".gtpl") {
            return Ok(None);
        }

        // Get document text
        let text = {
            let docs = self.documents.read().await;
            match docs.get(uri) {
                Some(t) => t.clone(),
                None => return Ok(None),
            }
        };

        // Parse template AST
        let template = match parse_template_file(&text, uri_str) {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };

        // Span uses 1-indexed; LSP position is 0-indexed.
        let cursor_line = position.line as u32 + 1;
        let cursor_col = position.character as u32 + 1;

        // Check if cursor is on a module_ref attribute span
        for resource in &template.resources {
            let mod_span = &resource.value.module_ref.span;
            if cursor_line >= mod_span.start_line
                && cursor_line <= mod_span.end_line
                && cursor_col >= mod_span.start_col
                && cursor_col <= mod_span.end_col
            {
                let module_name = &resource.value.module_ref.value;
                let root = {
                    let r = self.root_uri.read().await;
                    r.clone()
                };
                let gmod_path = find_gmod_file(&root, module_name)?;
                if let Some(path) = gmod_path {
                    let target_uri_str = format!("file://{}", path.to_string_lossy());
                    let target_uri = match target_uri_str.parse::<Uri>() {
                        Ok(u) => u,
                        Err(_) => return Ok(None),
                    };
                    return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                        uri: target_uri,
                        range: Range {
                            start: Position { line: 0, character: 0 },
                            end: Position { line: 0, character: 0 },
                        },
                    })));
                }
            }
        }

        Ok(None)
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

/// Extract the parent directory of a file:// URI as a file:// URI string, for use as a
/// fallback search root when no workspace rootUri was provided at initialization.
fn uri_to_parent_dir(uri_str: &str) -> Option<String> {
    let path_str = uri_str.strip_prefix("file://")?;
    let path = std::path::Path::new(path_str);
    let parent = path.parent()?;
    Some(format!("file://{}", parent.to_string_lossy()))
}

/// Search workspace root recursively for a .gmod file matching `module_name`.
/// Returns the first match found, or None if not found or root is unknown.
fn find_gmod_file(
    root_uri: &Option<String>,
    module_name: &str,
) -> LspResult<Option<std::path::PathBuf>> {
    let root_str = match root_uri {
        Some(r) => r,
        None => return Ok(None),
    };
    // Strip "file://" prefix (handles both file:/// and file://)
    let root_path = root_str
        .strip_prefix("file://")
        .unwrap_or(root_str.as_str());
    let root = std::path::Path::new(root_path);
    if !root.exists() {
        return Ok(None);
    }
    let target_filename = format!("{}.gmod", module_name);
    find_gmod_recursive(root, &target_filename, 0)
}

fn find_gmod_recursive(
    dir: &std::path::Path,
    target: &str,
    depth: usize,
) -> LspResult<Option<std::path::PathBuf>> {
    if depth > 5 {
        return Ok(None); // max depth guard
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if path.file_name().and_then(|n| n.to_str()) == Some(target) {
                return Ok(Some(path));
            }
        } else if path.is_dir() {
            if let Some(found) = find_gmod_recursive(&path, target, depth + 1)? {
                return Ok(Some(found));
            }
        }
    }
    Ok(None)
}

/// Extract the identifier word at the given 0-indexed line and character position.
fn extract_word_at_position(text: &str, line: usize, character: usize) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let line_str = lines.get(line)?;
    let chars: Vec<char> = line_str.chars().collect();
    let pos = character.min(chars.len());

    // Scan left to find start of word
    let mut start = pos;
    while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        start -= 1;
    }
    // Scan right to find end of word
    let mut end = pos;
    while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
        end += 1;
    }
    if start == end {
        return None;
    }
    Some(chars[start..end].iter().collect())
}

/// Static lookup of governance metadata field descriptions for hover.
fn governance_hover(field_name: &str) -> Option<&'static str> {
    match field_name {
        "security_tier" => Some(
            "**security_tier** — Security classification of this module.\n\nAccepted values: `\"critical\"`, `\"high\"`, `\"medium\"`, `\"low\"`",
        ),
        "compliance_frameworks" => Some(
            "**compliance_frameworks** — Applicable compliance frameworks.\n\nExample: `\"soc2,pci-dss,hipaa\"`",
        ),
        "cost_tier" => Some(
            "**cost_tier** — Cost category of this module.\n\nAccepted values: `\"xl\"`, `\"l\"`, `\"m\"`, `\"s\"`, `\"xs\"`",
        ),
        "data_classification" => Some(
            "**data_classification** — Data sensitivity classification.\n\nAccepted values: `\"sensitive\"`, `\"internal\"`, `\"public\"`",
        ),
        "disaster_recovery_tier" => Some(
            "**disaster_recovery_tier** — Disaster recovery tier requirement.\n\nExample: `\"none\"`, `\"warm\"`, `\"hot\"`",
        ),
        "approval_required" => Some(
            "**approval_required** — Whether this module requires approver sign-off (EE only).\n\nAccepted values: `true`, `false`",
        ),
        _ => None,
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
                root_uri: Arc::new(RwLock::new(None)),
            });
            Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
                .serve(service)
                .await;
        });
}
