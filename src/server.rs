use serde::de::value;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::{InputEdit, Query, QueryCursor};

use crate::json;
use crate::lsp::{Document, Documents};
use crate::parser;
use crate::tree;

use streaming_iterator::StreamingIterator;

/*
 TODO:
* Handle Pseudo instructions
* Store labels and use them for goto_definition
* Distinguish between text & data sections
*/

#[derive(Debug)]
struct Backend {
    client: Client,
    // params: InitializeParams,
    documents: Documents,
    instructions: json::Instructions,
    directives: json::Directives,
    registers: json::Registers,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(options) = params.initialization_options {
            log!("Received initialization options:");
            log!("{}", options);
        }

        Ok(InitializeResult {
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
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        log!("Server initialized");
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        log!("workspace/didChangeConfiguration");

        let settings = params.settings;
        log!("nopts {}", settings);
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        log!("textDocument/didOpen");
        log!(
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
        self.documents
            .insert(uri.clone(), Document { text, tree });

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
                        self.long_instruction_docs(cursor_node_text.to_string(), instruction);

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

    /**
     * Returns a long description of an instruction as a String.
     * Prepends a short description with the instruction format.
     */
    fn long_instruction_docs(&self, opcode: String, instruction: &json::Instruction) -> String {
        let instruction_format = &instruction.format;
        format!(
            "{}: `{}`\n---\n{}",
            if instruction_format.is_empty() {
                "Pseudo Instruction".to_string()
            } else {
                format!("{} Format Instruction", instruction_format)
            },
            opcode,
            self.short_instruction_docs(opcode.as_str(), instruction)
        )
    }

    /**
     * Returns a short description of an instruction as a String.
     */
    fn short_instruction_docs(&self, opcode: &str, instruction: &json::Instruction) -> String {
        let native_instruction = !instruction.native.is_empty();
        let pseudo_instruction = !instruction.pseudo.is_empty();

        let mut docs = "".to_string();

        if native_instruction {
            for variant in &instruction.native {
                docs = format!(
                    "{}\n{}\n```asm\n{} {}\n```\n",
                    docs,
                    variant.description,
                    opcode,
                    variant.operands.join(", "),
                );
            }
        }

        if pseudo_instruction {
            if native_instruction {
                docs = format!("{}\n---\n**Pseudo Instruction Alternative**\n&nbsp;", docs);
            }

            for variant in &instruction.pseudo {
                docs = format!(
                    "{}\n{}\n```asm\n{} {}\n```",
                    docs,
                    variant.description,
                    opcode,
                    variant.operands.join(", "),
                );
            }
        }
        docs
    }
}

pub async fn serve() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;

    log!("Stopping MIPS language server");
}
