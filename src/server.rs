use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::{Mutex, RwLock};

use serde::de::value;
use streaming_iterator::StreamingIterator;
use tower_lsp_server::jsonrpc;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use tree_sitter::{InputEdit, Query, QueryCursor};

use crate::ast;
use crate::completion;
use crate::document;
use crate::document::Document;
use crate::goto_definition;
use crate::hover;
use crate::lang::LanguageDefinitions;
use crate::semantic;
use crate::settings::Settings;

pub type Documents = dashmap::DashMap<tower_lsp_server::ls_types::Uri, Arc<RwLock<Document>>>;

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
        definition_provider: Some(OneOf::Left(true)),
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
                change: Some(TextDocumentSyncKind::INCREMENTAL),
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
        }

        let unpacked_settings = &self.settings.read().await;
        self.definitions.write().await.parse(unpacked_settings);

        Ok(InitializeResult {
            server_info: Some(get_server_info()),
            capabilities: get_server_capabilities(),
            offset_encoding: Some("utf-16".into()),
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

        let mips_config = params.settings.get("mipsls").cloned().unwrap_or_default();

        let result = self.settings.write().await.parse(mips_config);
        if let Err(e) = result {
            log!("Got settings error: {}", e);
            return;
        }

        let unpacked_settings = &self.settings.read().await;

        self.definitions.write().await.parse(unpacked_settings);
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

        let mut document = Document::new(uri.clone(), version, text);

        // Analyze document and publish diagnostics
        let diagnostics = document.analyze().await;
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;

        // Store document text and tree
        self.documents.insert(uri, Arc::new(RwLock::new(document)));
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
                if let Some(range) = change.range {
                    let range = doc.ls_range_to_ts(&range);
                    doc.apply_change(&change.text, range);
                } else {
                    doc.parse_entire_document(&change.text);
                }
            }

            // Analyze document and publish diagnostics
            let diagnostics = doc.analyze().await;
            self.client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        }
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> jsonrpc::Result<Option<CompletionResponse>> {
        self.get_completions(params).await
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> jsonrpc::Result<Option<GotoDefinitionResponse>> {
        self.do_goto_definition(params).await
    }

    async fn hover(&self, params: HoverParams) -> jsonrpc::Result<Option<Hover>> {
        self.handle_hover(params).await
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
}

pub async fn serve() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;

    log!("Stopping MIPS language server");
}
