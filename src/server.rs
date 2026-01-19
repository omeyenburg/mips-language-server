use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::{Mutex, RwLock};

use serde::de::value;
use streaming_iterator::StreamingIterator;
use tower_lsp_server::jsonrpc;
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use tree_sitter::{InputEdit, Query, QueryCursor};

use crate::completion;
use crate::hover;
use crate::lang::LanguageDefinitions;
use crate::settings::Settings;
use crate::tree;

pub struct Document {
    pub version: i32,
    pub text: String,
    pub tree: tree_sitter::Tree,
    pub parser: tree_sitter::Parser,
}

pub type Documents = dashmap::DashMap<tower_lsp_server::lsp_types::Uri, Arc<RwLock<Document>>>;

/*
 TODO:
* Handle Pseudo instructions
* Store labels and use them for goto_definition
* Distinguish between text & data sections
*/

pub struct Backend {
    pub client: Client,
    pub settings: RwLock<Settings>,
    pub documents: Documents,
    pub definitions: RwLock<LanguageDefinitions>,
}

fn get_server_info() -> ServerInfo {
    ServerInfo {
        name: "mips-language-server".to_string(),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
    }
}

fn get_server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        // inlay_hint_provider: Some(OneOf::Left(true)),
        // definition_provider: Some(OneOf::Left(true)),
        // references_provider: Some(OneOf::Left(true)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(false),
            trigger_characters: Some(vec![".".to_string(), "$".to_string()]),
            work_done_progress_options: Default::default(),
            all_commit_characters: None,
            completion_item: None,
        }),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                // change: Some(TextDocumentSyncKind::FULL),
                change: Some(TextDocumentSyncKind::INCREMENTAL), // only tree is incremental
                save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                    include_text: Some(true),
                })),
                ..Default::default()
            },
        )),
        // Enables client diagnostic pulling, but we should prefer diagnostic pushing
        // diagnostic_provider: Some(DiagnosticServerCapabilities::Options({ DiagnosticOptions { identifier: None, inter_file_dependencies: false, workspace_diagnostics: true, work_done_progress_options: WorkDoneProgressOptions { work_done_progress: None, }, } })),

        // Maybe implement semantic token provider for vscode
        // semantic_tokens_provider: Some( SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions( SemanticTokensRegistrationOptions { text_document_registration_options: { TextDocumentRegistrationOptions { document_selector: Some(vec![DocumentFilter { language: Some("asm".to_string()), scheme: Some("file".to_string()), pattern: None, }]), } }, semantic_tokens_options: SemanticTokensOptions { work_done_progress_options: WorkDoneProgressOptions::default(), legend: SemanticTokensLegend { token_types: LEGEND_TYPE.into(), token_modifiers: vec![], }, range: Some(true), full: Some(SemanticTokensFullOptions::Bool(true)), }, static_registration_options: StaticRegistrationOptions::default(), },),),
        ..ServerCapabilities::default()
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        log!("Starting MIPS language server");

        if let Some(settings) = params.initialization_options {
            self.settings
                .write()
                .await
                .parse(settings)
                .map_err(|e| jsonrpc::Error::invalid_params(e.to_string()))?;

            self.definitions
                .write()
                .await
                .parse(&*&self.settings.read().await);
        }

        Ok(InitializeResult {
            server_info: Some(get_server_info()),
            capabilities: get_server_capabilities(),
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        log!("Server initialized");
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        log!("workspace/didChangeConfiguration");

        let mips_config = params
            .settings
            .get("mipsls")
            .cloned()
            .unwrap_or_default();

        let result = self.settings.write().await.parse(mips_config);
        if let Err(e) = result {
            log!("Got settings error: {}", e);
            return;
        }

        self.definitions
            .write()
            .await
            .parse(&*&self.settings.read().await);
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        log!("textDocument/didOpen");
        // log!(
        //     "{:?}",
        //     params.text_document.uri.path() // might panic - dont call in prod
        // );

        let TextDocumentItem {
            uri, text, version, ..
        } = params.text_document;

        let mut parser = tree::create_parser();

        // Generate tree using tree-sitter
        let tree = parser
            .parse(text.as_bytes(), None)
            .ok_or(jsonrpc::Error::invalid_request())
            .expect("Expected new tree");

        let document = Document {
            version,
            text,
            tree,
            parser,
        };

        // Store document text and tree
        self.documents
            .insert(uri.clone(), Arc::new(RwLock::new(document)));

        // Analyze document and publish diagnostics
        let diags = self.analyze_document(&uri).await;
        self.client.publish_diagnostics(uri, diags, None).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let changes = params.content_changes;

        if let Some(doc_arc) = self.documents.get_mut(&uri) {
            let mut doc = doc_arc.write().await;

            if version <= doc.version {
                // Skip older edits when changes are received out of order.
                return;
            }
            doc.version = version;

            // TODO: Consider rewriting this.
            // This assumes that all changes happen in order and are never skipped.
            // Alternative: use queue ordered by version; if version+3 comes in while waiting for
            // version+1, consider reparsing the document fully
            for change in changes {
                Backend::handle_change(&mut *doc, change);
            }
        }

        // Analyze document and publish diagnostics
        let diags = self.analyze_document(&uri).await;
        self.client.publish_diagnostics(uri, diags, None).await;
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> jsonrpc::Result<Option<CompletionResponse>> {
        self.get_completions(params).await
    }

    async fn hover(&self, params: HoverParams) -> jsonrpc::Result<Option<Hover>> {
        self.hover(params).await
    }

    // Used for diagnostic pulling, but we prefer pushing model
    // async fn diagnostic(
    //     &self,
    //     params: DocumentDiagnosticParams,
    // ) -> jsonrpc::Result<DocumentDiagnosticReportResult> {
    //     Ok(DocumentDiagnosticReportResult::Report(
    //         DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
    //             full_document_diagnostic_report: FullDocumentDiagnosticReport {
    //                 items: vec![],
    //                 result_id: None,
    //             },
    //             related_documents: None,
    //         }),
    //     ))
    // }
}

impl Backend {
    pub fn new(client: Client) -> Self {
        let documents = dashmap::DashMap::new();

        let default_settings = Settings::default();
        let default_definitions = LanguageDefinitions::new();

        let settings = RwLock::new(default_settings);
        let definitions = RwLock::new(default_definitions);

        Self {
            client,
            settings,
            documents,
            definitions,
        }
    }

    fn handle_change(doc: &mut Document, change: TextDocumentContentChangeEvent) {
        if let Some(range) = change.range {
            // Convert LSP range to byte offsets
            let start_byte = tree::byte_offset(&doc.text, range.start);
            let old_end_byte = tree::byte_offset(&doc.text, range.end);
            let new_end_byte = start_byte + change.text.len();

            // Stop the server from crashing here
            // Weird bug, maybe happens with multiple changes
            if old_end_byte >= doc.text.len() {
                log!("Error: something wrong with the bytes but idk - please write an issue on https://github.com/omeyenburg/mips-language-server");
                return;
            }

            // Calculate new text, old text and apply change to text
            let old_text = &doc.text[start_byte..old_end_byte];
            doc.text
                .replace_range(start_byte..old_end_byte, &change.text);

            // Create an InputEdit
            let input_edit = InputEdit {
                start_byte,
                old_end_byte,
                new_end_byte,
                start_position: tree::position_to_point(&range.start),
                old_end_position: tree::position_to_point(&range.end),
                new_end_position: tree::position_to_point(&Position {
                    line: range.start.line + change.text.lines().count() as u32, // - 1, // stupid to substract 1. why?!
                    character: if let Some(last_line) = change.text.lines().last() {
                        last_line.len() as u32
                    } else {
                        range.start.character
                    },
                }),
            };

            // Apply InputEdit to the tree
            doc.tree.edit(&input_edit);

            // Update the tree incrementally
            let new_tree = doc.parser.parse(&doc.text, Some(&doc.tree)).unwrap();
            doc.tree = new_tree;
        } else {
            // Full text replacement
            doc.text = change.text.clone();
            doc.tree = doc.parser.parse(&doc.text, None).unwrap();
        }
    }
}

pub async fn serve() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;

    log!("Stopping MIPS language server");
}
