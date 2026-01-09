//! utf8proj Language Server
//!
//! Provides IDE support for .proj files:
//! - Real-time diagnostics (errors, warnings, hints)
//! - Hover information (profile resolution, cost ranges)
//! - Document symbols (tasks, resources, profiles)

mod diagnostics;
mod hover;
mod navigation;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use utf8proj_core::{CollectingEmitter, Diagnostic as CoreDiagnostic, Project, Schedule, Scheduler};
use utf8proj_parser::parse_project;
use utf8proj_solver::{analyze_project, AnalysisConfig, CpmSolver};

use crate::diagnostics::to_lsp_diagnostics;
use crate::hover::get_hover_info;

/// Document state cached by the server
#[derive(Debug, Default)]
struct DocumentState {
    /// Raw document text
    text: String,
    /// Parsed project (if successful)
    project: Option<Project>,
    /// Computed schedule (if successful)
    schedule: Option<Schedule>,
    /// Parse error (if failed)
    parse_error: Option<String>,
    /// Collected diagnostics for hover info
    diagnostics: Vec<CoreDiagnostic>,
}

/// The utf8proj language server
struct Backend {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, DocumentState>>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Analyze a document and publish diagnostics
    async fn analyze_document(&self, uri: Url, text: String) {
        let mut state = DocumentState {
            text: text.clone(),
            project: None,
            schedule: None,
            parse_error: None,
            diagnostics: Vec::new(),
        };

        // Try to parse the document
        let diagnostics = match parse_project(&text) {
            Ok(project) => {
                // Parse succeeded - run semantic analysis
                let solver = CpmSolver::new();
                let schedule = solver.schedule(&project).ok();

                let mut emitter = CollectingEmitter::new();
                let config = AnalysisConfig::new().with_file(uri.path());
                analyze_project(&project, schedule.as_ref(), &config, &mut emitter);

                // Store diagnostics for hover info
                state.diagnostics = emitter.diagnostics.clone();
                state.schedule = schedule;
                state.project = Some(project);
                to_lsp_diagnostics(&emitter.diagnostics)
            }
            Err(e) => {
                // Parse failed - emit parse error as diagnostic
                state.parse_error = Some(e.to_string());
                vec![Diagnostic {
                    range: Range::new(Position::new(0, 0), Position::new(0, 1)),
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("parse-error".to_string())),
                    source: Some("utf8proj".to_string()),
                    message: e.to_string(),
                    ..Default::default()
                }]
            }
        };

        // Update document state
        {
            let mut docs = self.documents.write().await;
            docs.insert(uri.clone(), state);
        }

        // Publish diagnostics
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "utf8proj-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "utf8proj language server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.analyze_document(uri, text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        // We use FULL sync, so there's exactly one change with the full text
        if let Some(change) = params.content_changes.into_iter().next() {
            self.analyze_document(uri, change.text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        // Clear diagnostics and remove document state
        self.client.publish_diagnostics(uri.clone(), vec![], None).await;
        let mut docs = self.documents.write().await;
        docs.remove(&uri);
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        if let Some(state) = docs.get(&uri) {
            if let Some(ref project) = state.project {
                return Ok(get_hover_info(
                    project,
                    state.schedule.as_ref(),
                    &state.diagnostics,
                    &state.text,
                    position,
                ));
            }
        }

        Ok(None)
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;

        let docs = self.documents.read().await;
        if let Some(state) = docs.get(&uri) {
            if let Some(ref project) = state.project {
                let symbols = build_document_symbols(project);
                return Ok(Some(DocumentSymbolResponse::Flat(symbols)));
            }
        }

        Ok(None)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        if let Some(state) = docs.get(&uri) {
            if let Some(ref project) = state.project {
                if let Some(location) =
                    navigation::find_definition(project, &state.text, &uri, position)
                {
                    return Ok(Some(GotoDefinitionResponse::Scalar(location)));
                }
            }
        }

        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let include_declaration = params.context.include_declaration;

        let docs = self.documents.read().await;
        if let Some(state) = docs.get(&uri) {
            if let Some(ref project) = state.project {
                let locations = navigation::find_references(
                    project,
                    &state.text,
                    &uri,
                    position,
                    include_declaration,
                );
                if !locations.is_empty() {
                    return Ok(Some(locations));
                }
            }
        }

        Ok(None)
    }
}

/// Build document symbols from a parsed project
fn build_document_symbols(project: &Project) -> Vec<SymbolInformation> {
    let mut symbols = Vec::new();

    // Add profiles
    for profile in &project.profiles {
        #[allow(deprecated)] // SymbolInformation is deprecated in LSP 3.17 but still widely used
        symbols.push(SymbolInformation {
            name: profile.id.clone(),
            kind: SymbolKind::CLASS,
            tags: None,
            deprecated: None,
            location: Location {
                uri: Url::parse("file:///").unwrap(),
                range: Range::default(),
            },
            container_name: Some("profiles".to_string()),
        });
    }

    // Add resources
    for resource in &project.resources {
        #[allow(deprecated)]
        symbols.push(SymbolInformation {
            name: resource.id.clone(),
            kind: SymbolKind::VARIABLE,
            tags: None,
            deprecated: None,
            location: Location {
                uri: Url::parse("file:///").unwrap(),
                range: Range::default(),
            },
            container_name: Some("resources".to_string()),
        });
    }

    // Add tasks (flattened)
    fn add_tasks(tasks: &[utf8proj_core::Task], prefix: &str, symbols: &mut Vec<SymbolInformation>) {
        for task in tasks {
            let name = if prefix.is_empty() {
                task.id.clone()
            } else {
                format!("{}.{}", prefix, task.id)
            };

            let kind = if task.milestone {
                SymbolKind::EVENT
            } else if !task.children.is_empty() {
                SymbolKind::MODULE
            } else {
                SymbolKind::FUNCTION
            };

            #[allow(deprecated)]
            symbols.push(SymbolInformation {
                name: name.clone(),
                kind,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: Url::parse("file:///").unwrap(),
                    range: Range::default(),
                },
                container_name: Some("tasks".to_string()),
            });

            if !task.children.is_empty() {
                add_tasks(&task.children, &name, symbols);
            }
        }
    }

    add_tasks(&project.tasks, "", &mut symbols);

    symbols
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
