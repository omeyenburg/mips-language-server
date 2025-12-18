use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::Mutex;

use serde::de::value;
use tower_lsp_server::jsonrpc;
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use tree_sitter::{InputEdit, Query, QueryCursor};

use crate::completion;
use crate::hover;

use crate::language_definitions::LanguageDefinitions;
use crate::lsp::{Document, Documents};
use crate::settings::Settings;
use crate::tree;

use streaming_iterator::StreamingIterator;

/*
 TODO:
* Handle Pseudo instructions
* Store labels and use them for goto_definition
* Distinguish between text & data sections
*/

#[derive(Debug)]
pub struct Backend {
    pub client: Client,
    pub settings: OnceLock<Settings>,
    pub documents: Documents,
    pub definitions: OnceLock<LanguageDefinitions>,
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        if let Some(options) = params.initialization_options.clone() {
            log!("Received initialization options:");
            log!("{}", options);
        }

        let settings = Settings::new(params.initialization_options)
            .map_err(|e| jsonrpc::Error::invalid_params(e.to_string()))?;


        let definitions = LanguageDefinitions::new(&settings);

        // TODO: handle
        let _ = self.settings.set(settings);
        let _ = self.definitions.set(definitions);

        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "mips-language-server".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            capabilities: ServerCapabilities {
                // inlay_hint_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                // definition_provider: Some(OneOf::Left(true)),
                // references_provider: Some(OneOf::Left(true)),
                diagnostic_provider: Some(DiagnosticServerCapabilities::Options({
                    DiagnosticOptions {
                        identifier: None,
                        inter_file_dependencies: true,
                        workspace_diagnostics: true,
                        work_done_progress_options: WorkDoneProgressOptions {
                            work_done_progress: None,
                        },
                    }
                })),
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
                //semantic_tokens_provider: Some(
                //    SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                //        SemanticTokensRegistrationOptions {
                //            text_document_registration_options: {
                //                TextDocumentRegistrationOptions {
                //                    document_selector: Some(vec![DocumentFilter {
                //                        language: Some("asm".to_string()),
                //                        scheme: Some("file".to_string()),
                //                        pattern: None,
                //                    }]),
                //                }
                //            },
                //            semantic_tokens_options: SemanticTokensOptions {
                //                work_done_progress_options: WorkDoneProgressOptions::default(),
                //                legend: SemanticTokensLegend {
                //                    token_types: LEGEND_TYPE.into(),
                //                    token_modifiers: vec![],
                //                },
                //                range: Some(true),
                //                full: Some(SemanticTokensFullOptions::Bool(true)),
                //            },
                //            static_registration_options: StaticRegistrationOptions::default(),
                //        },
                //    ),
                //),
                ..ServerCapabilities::default()
            },
            ..Default::default()
        })
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> jsonrpc::Result<Option<CompletionResponse>> {
        completion::get_completions(self, params)
    }

    async fn hover(&self, params: HoverParams) -> jsonrpc::Result<Option<Hover>> {
        hover::hover(self, params)
    }

    async fn initialized(&self, _: InitializedParams) {
        log!("Server initialized");
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        log!("workspace/didChangeConfiguration");

        let settings = params.settings;
        log!("options {}", settings);
    }

    async fn diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> jsonrpc::Result<DocumentDiagnosticReportResult> {
        Ok(DocumentDiagnosticReportResult::Report(
            DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    items: vec![],
                    result_id: None,
                },
                related_documents: None,
            }),
        ))
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        log!("textDocument/didOpen");
        log!(
            "{:?}",
            params.text_document.uri.path() // might panic - dont call in prod
        );

        let TextDocumentItem { uri, text, .. } = params.text_document;

        // Generate tree using tree-sitter
        let tree = tree::create_parser()
            .parse(text.as_bytes(), None)
            .ok_or(jsonrpc::Error::invalid_request())
            .expect("Expected new tree");

        // Store document text and tree
        self.documents.insert(uri.clone(), Document { text, tree });

        // // Parse the document and generate diagnostics
        // let diagnostics = self.parse(&uri);

        // // Publish diagnostics
        // self.client
        //     .publish_diagnostics(uri, diagnostics, None)
        //     .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = &params.text_document.uri;
        let changes = params.content_changes;

        if let Some(mut document) = self.documents.get_mut(uri) {
            // Create parser, so we dont need to recreate it for every change
            //  TODO:(But Im also too lazy to store the parser globally)
            let mut parser = tree::create_parser();

            // Important debugging step
            if changes.len() != 1 {
                log!("WOW {} CHANGES!", changes.len());
            }

            for change in changes {
                if let Some(range) = change.range {
                    // Convert LSP range to byte offsets
                    let start_byte = tree::byte_offset(&document.text, range.start);
                    let old_end_byte = tree::byte_offset(&document.text, range.end);
                    let new_end_byte = start_byte + change.text.len();

                    // Stop the server from crashing here
                    // Weird bug, maybe happens with multiple changes
                    if old_end_byte >= document.text.len() {
                        log!("Error: something wrong with the bytes but idk");
                        return;
                    }

                    // Calculate new text, old text and apply change to text
                    let old_text = &document.text[start_byte..old_end_byte];
                    document
                        .text
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
                    document.tree.edit(&input_edit);

                    // Update the tree incrementally
                    let new_tree = parser.parse(&document.text, Some(&document.tree)).unwrap();
                    document.tree = new_tree;
                } else {
                    // Full text replacement
                    document.text = change.text.clone();
                    document.tree = parser.parse(&document.text, None).unwrap();
                }
            }
        }

        // // Parse the document and generate diagnostics
        // let diagnostics = self.parse(uri);

        // // Publish diagnostics
        // self.client
        //     .publish_diagnostics(uri.clone(), diagnostics, None)
        //     .await;
    }
}

impl Backend {
    pub fn new(client: Client) -> Self {
        let settings = OnceLock::new();
        let documents = dashmap::DashMap::new();
        let definitions = OnceLock::new();

        Self {
            client,
            settings,
            documents,
            definitions,
            // instructions,
            // directives,
            // registers,
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
