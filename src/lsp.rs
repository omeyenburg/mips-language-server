use crate::json;
use std::error::Error;

#[derive(Debug)]
pub struct Document {
    pub text: String,
    pub tree: tree_sitter::Tree,
}

#[cfg(feature = "sync")]
use lsp_types::*;

#[cfg(feature = "sync")]
type Documents = dashmap::DashMap<lsp_types::Uri, Document>;

#[cfg(feature = "async")]
type Documents = std::collections::HashMap<tower_lsp::lsp_types::Url, Document>;

#[cfg(feature = "async")]
use tower_lsp::lsp_types::*;

#[cfg(feature = "async")]
type Uri = Url;

#[derive(Debug)]
pub struct Workspace {
    pub documents: Documents,
    instructions: json::Instructions,
    directives: json::Directives,
    registers: json::Registers,
}

impl Workspace {
    pub fn get_server_capabilities(self) -> InitializeResult {
        InitializeResult {
            capabilities: ServerCapabilities {
                inlay_hint_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
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
                        //change: Some(TextDocumentSyncKind::FULL),
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
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
        }
    }

    pub fn completion(
        self,
        documents: &Documents,
        params: CompletionParams,
    ) -> Result<Option<CompletionResponse>, Box<dyn Error + Sync + Send>> {
        let line = params.text_document_position.position.line;
        let character = params.text_document_position.position.character;

        // Retrieve current content and tree
        let document = documents
            .get(&params.text_document_position.text_document.uri.into())
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())?;
        let text = &document.text;

        // Split the text into lines and retrieve the specific line
        let line_content = text
            .lines()
            .nth(line as usize)
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())?;

        // Find starting character of word that should be completed
        // Looks for '.' and '$' and returns ' ' if no match is found
        let mut starting_char = ' ';
        let mut starting_index = 0;
        for i in 0..character {
            starting_index = character - i - 1;
            if let Some(char) = line_content.chars().nth(starting_index as usize) {
                match char {
                    '$' | '.' => starting_char = char,
                    ' ' => {
                        // This is necessary to diffentiate from the loop
                        // ending, because the beginning of the line is reached
                        starting_index += 1;
                        break;
                    }
                    _ => starting_char = ' ',
                }
            }
        }

        // Generate completion items
        // Split up in: directive, register and instruction
        let items: Vec<CompletionItem> = match starting_char {
            '.' => self
                .directives
                .keys()
                .map(|keyword| CompletionItem {
                    label: keyword.to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    detail: Some(format!("MIPS directive: {}", keyword)),
                    label_details: Some(CompletionItemLabelDetails {
                        detail: None,
                        description: Some("directive".to_string()),
                    }),
                    documentation: Some(Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: self.directives.get(keyword).unwrap().to_string(),
                    })),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range: Range {
                            start: Position {
                                line,
                                character: starting_index,
                            },
                            end: Position { line, character },
                        },
                        new_text: keyword.to_string(),
                    })),
                    ..Default::default()
                })
                .collect(),

            '$' => {
                let mut registers_common_float = self.registers.common.clone();

                let registers = if character - starting_index == 1 {
                    // Return common & float
                    for (key, value) in &self.registers.float {
                        registers_common_float.insert(key.to_string(), value.to_string());
                    }
                    &registers_common_float
                } else if let Some(char) = line_content.chars().nth(starting_index as usize + 1) {
                    // Check context
                    match char {
                        '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' => {
                            &self.registers.numeric
                        }
                        'f' => &self.registers.float,
                        _ => &self.registers.common,
                    }
                } else {
                    // Return common & float
                    for (key, value) in &self.registers.float {
                        registers_common_float.insert(key.to_string(), value.to_string());
                    }
                    &registers_common_float
                };

                registers
                    .keys()
                    .map(|keyword| CompletionItem {
                        label: keyword.to_string(),
                        kind: Some(CompletionItemKind::VALUE),
                        detail: Some(format!("MIPS register: {}", keyword)),
                        label_details: Some(CompletionItemLabelDetails {
                            detail: None,
                            description: Some("register".to_string()),
                        }),
                        documentation: Some(Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: registers.get(keyword).unwrap().to_string(),
                        })),
                        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                            range: Range {
                                start: Position {
                                    line,
                                    character: starting_index,
                                },
                                end: Position { line, character },
                            },
                            new_text: keyword.to_string(),
                        })),
                        ..Default::default()
                    })
                    .collect()
            }

            _ => {
                let good_named_func = |keyword: &str| CompletionItem {
                    label: keyword.to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    detail: Some({
                        let instruction_format = &self.instructions.get(keyword).unwrap().format;
                        if instruction_format.is_empty() {
                            format!("Pseudo Instruction: {}", keyword)
                        } else {
                            format!("{} Format Instruction: {}", instruction_format, keyword)
                        }
                    }),
                    label_details: Some(CompletionItemLabelDetails {
                        detail: None,
                        description: Some("instruction".to_string()),
                    }),
                    documentation: Some(Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: self.short_instruction_docs(
                            keyword,
                            self.instructions.get(keyword).unwrap(),
                        ),
                    })),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range: Range {
                            start: Position {
                                line,
                                character: starting_index,
                            },
                            end: Position { line, character },
                        },
                        new_text: keyword.to_string(),
                    })),
                    ..Default::default()
                };

                // Completion is buggy when there is a dot.
                // This should filter instruction completions.
                if let Some(dot_position) = line_content[..character as usize].rfind(".") {
                    let prefix = &line_content[starting_index as usize..character as usize];
                    self.instructions
                        .keys()
                        .filter_map(|i| {
                            if i.starts_with(prefix) {
                                Some(good_named_func(
                                    &(prefix.to_owned()
                                        + &i[character as usize - starting_index as usize..]),
                                ))
                            } else {
                                None
                            }
                        })
                        .collect()
                } else {
                    self.instructions
                        .keys()
                        .map(|keyword| good_named_func(keyword))
                        .collect()
                }
            }
        };

        // Return the completion items
        Ok(Some(CompletionResponse::List(
            tower_lsp::lsp_types::CompletionList {
                is_incomplete: false,
                items,
            },
        )))
    }
}
