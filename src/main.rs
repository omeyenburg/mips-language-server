#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use crate::types::*;
use serde::de::value;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tracing::info;
use tree_sitter::{InputEdit, Query, QueryCursor};

use streaming_iterator::StreamingIterator;

mod json;
mod parser;
mod tree;

/*
 TODO:
* Handle Pseudo instructions
* Store labels and use them for goto_definition
* Distinguish between text & data sections
*/

mod types {
    #[derive(Debug)]
    pub struct Document {
        pub text: String,
        pub tree: tree_sitter::Tree,
    }
}

#[derive(Debug)]
struct Backend {
    client: Client,
    documents: dashmap::DashMap<Url, Document>,
    instructions: json::Instructions,
    directives: json::Directives,
    registers: json::Registers,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(options) = params.initialization_options {
            info!("Received initialization options:");
            info!("{}", options);
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                inlay_hint_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
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
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        info!("Server initialized");
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        info!("workspace/didChangeConfiguration");

        let settings = params.settings;
        info!("nopts {}", settings);
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        info!("textDocument/didOpen");
        info!(
            "{:?}",
            params
                .text_document
                .uri
                .to_file_path()
                .expect("expected file")
        );

        let TextDocumentItem { uri, text, .. } = params.text_document;

        // Generate tree using tree-sitter
        let tree = tree::create_parser()
            .parse(text.as_bytes(), None)
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())
            .expect("Expected new tree");

        // Store document text and tree
        self.documents.insert(uri.clone(), Document { text, tree });

        // Parse the document and generate diagnostics
        let diagnostics = self.parse(&uri);

        // Publish diagnostics
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
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
                info!("WOW {} CHANGES!", changes.len());
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
                        info!("Error: something wrong with the bytes but idk");
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

        // Parse the document and generate diagnostics
        let diagnostics = self.parse(uri);

        // Publish diagnostics
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let line = params.text_document_position.position.line;
        let character = params.text_document_position.position.character;

        // Retrieve current content and tree
        let document = self
            .documents
            .get(&params.text_document_position.text_document.uri)
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
                    detail: Some(format!(
                        "MIPS directive: {}",
                        self.directives.get(keyword).unwrap()
                    )),
                    label_details: Some(CompletionItemLabelDetails {
                        detail: None,
                        description: Some("directive".to_string()),
                    }),
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
                    // check_context
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
                        detail: Some(format!(
                            "MIPS register: {}",
                            registers.get(keyword).unwrap()
                        )),
                        label_details: Some(CompletionItemLabelDetails {
                            detail: None,
                            description: Some("register".to_string()),
                        }),
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
                    detail: Some("keyword".to_string()),
                    label_details: Some(CompletionItemLabelDetails {
                        detail: None,
                        description: Some("keyword".to_string()),
                    }),
                    documentation: Some(Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: self.get_instruction_docs(
                            keyword.to_string(),
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
                // This should fix this
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

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        // Unpack position and text_document
        let TextDocumentPositionParams {
            position,
            text_document,
        } = params.text_document_position_params;

        // Retrieve current content and tree
        let document = self
            .documents
            .get(&text_document.uri)
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())?;
        let text = &document.text;
        let tree = document.tree.clone();

        // Determine node below cursor and fetch the label name
        let point = tree::position_to_point(&position);
        let cursor_node = tree
            .root_node()
            .descendant_for_point_range(point, point)
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())?;

        // Get label kind and text
        let kind = cursor_node.kind();
        let cursor_node_text = cursor_node.utf8_text(text.as_bytes()).unwrap_or_default();

        match kind {
            "opcode" => {
                if let Some(instruction) = self.instructions.get(cursor_node_text) {
                    let documentation =
                        self.get_instruction_docs(cursor_node_text.to_string(), instruction);

                    return Ok(Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::String(documentation)),
                        range: None,
                    }));
                }
            }
            "meta" => {
                if let Some(directive) = self.directives.get(cursor_node_text) {
                    return Ok(Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::String(String::from(
                            directive,
                        ))),
                        range: None,
                    }));
                }
            }
            "register" => {
                if let Some(register) = self.registers.numeric.get(cursor_node_text) {
                    //if let Some(register) = self.registers.get("numeric").unwrap().get(cursor_node_text) {
                    return Ok(Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::String(String::from(
                            register,
                        ))),
                        range: None,
                    }));
                }
                if let Some(register) = self.registers.common.get(cursor_node_text) {
                    //if let Some(register) = self.registers.get("common").unwrap().get(cursor_node_text) {
                    return Ok(Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::String(String::from(
                            register,
                        ))),
                        range: None,
                    }));
                }
                if let Some(register) = self.registers.float.get(cursor_node_text) {
                    //if let Some(register) = self.registers.get("float").unwrap().get(cursor_node_text) {
                    return Ok(Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::String(String::from(
                            register,
                        ))),
                        range: None,
                    }));
                }
            }
            _ => {}
        }

        Ok(None)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        // Goto definition: searches for word below cursor among all labels

        // Unpack position and text_document
        let TextDocumentPositionParams {
            position,
            text_document,
        } = params.text_document_position_params;

        // Retrieve current content and tree
        let document = self
            .documents
            .get(&text_document.uri)
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())?;
        let text = &document.text;
        let tree = &document.tree;

        // Determine node below cursor and fetch the label name
        let point = tree::position_to_point(&position);
        let cursor_label_name = tree
            .root_node()
            .descendant_for_point_range(point, point)
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())?
            .utf8_text(text.as_bytes())
            .unwrap_or_default();

        // Generate query to find labels
        let query = Query::new(&tree_sitter_mips::language(), r#"(label) @label"#)
            .expect("Error compiling query");

        // Execute the query
        let mut query_cursor = QueryCursor::new();
        let mut matches = query_cursor.matches(&query, tree.root_node(), text.as_bytes());

        // Iterate over the matches
        while let Some(m) = matches.next() {
            if let Some(capture) = m.captures.first() {
                let node = capture.node;
                let label_text = &text[node.start_byte()..node.end_byte() - 1];

                if label_text == cursor_label_name {
                    return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                        uri: text_document.uri,
                        range: Range {
                            start: tree::point_to_position(&node.start_position()),
                            end: tree::point_to_position(&node.end_position()),
                        },
                    })));
                }
            }
        }

        // Return None; no definition found
        Ok(None)
    }
}

impl Backend {
    pub fn new(client: Client) -> Self {
        let documents = dashmap::DashMap::new();
        let instructions = json::read_instructions();
        let directives = json::read_directives();
        let registers = json::read_registers();

        Self {
            client,
            documents,
            instructions,
            directives,
            registers,
        }
    }

    fn parse(&self, uri: &Url) -> Vec<Diagnostic> {
        /* Parse the document (for now not incrementally)

        value types:
        [register]: $t0, $s1, $a0, $v0
        [float register]: $f0, $f1, $f31, $f12
        [immediate]: 24, 2134, 0x3fa, 0274124, 94967296
        [address]: x, y, x($t0), 4($t0), 0xa8($s1)
        [jump label]: main, loop

        - find sections
            - data
                - find variable labels
                    - lint: duplicate labels
                    - check syntax
                    - for arguments, check if numbers are in correct ranges
            - text
                - find jump labels
                    - lint: duplicate labels
                - process all instructions
                    - check syntax
                    - maybe give corrections using pseudo instructions
                    - check if immediate values are in correct ranges
                */

        // Vector of diagnostics that is published at the end
        let mut diagnostics = Vec::new();

        // Retrieve current content and tree
        let document = &self
            .documents
            .get(uri)
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())
            .unwrap();

        parser::parse_directives(&mut diagnostics, document, &self.directives);
        parser::parse_instructions(&mut diagnostics, document, &self.instructions);
        parser::parse_labels(&mut diagnostics, document);

        diagnostics
    }

    /*
    Returns a description of an instruction as a String.
    Format:
    [...] Type Instruction
    –––
    ```asm
    [opcode] [operands]
    ```
    [description]
    [machine code]
    –––
    ```asm
    [opcode] [operands]
    ```
    [description]
    [machine code]
    */
    fn get_instruction_docs(&self, opcode: String, instruction: &json::Instruction) -> String {
        if instruction.format.is_empty() {
            // Pure pseudo instruction
            let mut docs = "# Pseudo Instruction\n---\n".to_string();

            for variant in &instruction.pseudo {
                docs = format!(
                    "{}\n```asm\n{} {}\n```\n{}\n",
                    docs,
                    opcode,
                    variant.operands.join(", "),
                    variant.description
                );
            }
            docs
        } else if instruction.pseudo.is_empty() {
            // Pure native instruction
            let mut docs = format!("# {} Format Instruction\n", instruction.format);

            for variant in &instruction.native {
                docs = format!(
                    "{}\n```asm\n{} {}\n```\n{}\n",
                    docs,
                    opcode,
                    variant.operands.join(", "),
                    variant.description
                );
            }
            docs
        } else {
            // Mixed instruction
            let mut docs = format!("# {} Format Instruction\n", instruction.format);

            for variant in &instruction.native {
                docs = format!(
                    "{}\n```asm\n{} {}\n```\n{}\n",
                    docs,
                    opcode,
                    variant.operands.join(", "),
                    variant.description
                );
            }

            docs = format!("{}## Pseudo Instruction Alternative\n", docs);

            for variant in &instruction.pseudo {
                docs = format!(
                    "{}\n```asm\n{} {}\n```\n{}\n",
                    docs,
                    opcode,
                    variant.operands.join(", "),
                    variant.description
                );
            }
            docs
        }
    }
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args();
    match args.nth(1).as_deref() {
        None => {
            // Set up log file
            let log_file = std::fs::File::create("/home/oskar/git/mips-language-server/lsp.log")
                .expect("Create file");
            let log_file = std::io::BufWriter::new(log_file);
            let (non_blocking, _guard) = tracing_appender::non_blocking(log_file);
            let subscriber = tracing_subscriber::fmt()
                .with_max_level(tracing::level_filters::LevelFilter::DEBUG)
                .with_writer(non_blocking)
                .without_time() // Compact log messages
                .with_level(false)
                .with_target(false)
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .expect("Could not set default subscriber");
            info!("Starting mips-language-server");

            let stdin = tokio::io::stdin();
            let stdout = tokio::io::stdout();

            let (service, socket) = LspService::new(Backend::new);
            Server::new(stdin, stdout, socket).serve(service).await;
        }
        Some("--version") => {
            println!("mips-language-server v0.1.0");
        }
        Some(_) => {
            println!("Usage:\n  mips-language-server [options]\n\nOptions:\n  --version, -v         Version");
        }
    };
}
