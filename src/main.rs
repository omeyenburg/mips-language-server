#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use crate::types::*;
use std::ops::Deref;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tracing::info;
use tree_sitter::{InputEdit, Parser, Query, QueryCursor, Tree};

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
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
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

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        info!(
            "textDocument/didOpen: {:?}",
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
        //let mut keywords = Vec::new();
        //keywords.extend(self.instructions.keys().cloned());
        //keywords.extend(self.directives.keys().cloned());
        //keywords.extend(self.registers.keys().cloned());

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
                    //_ => {}
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
                let good_named_func = |keyword: &str| CompletionItem {
                    label: keyword.to_string(),
                    kind: Some(CompletionItemKind::VALUE),
                    detail: Some(format!(
                        "MIPS register: {}",
                        self.registers.get(keyword).unwrap().to_string()
                    )),
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
                if character - starting_index == 1 {
                    self.registers
                        .keys()
                        .filter(|r| {
                            r.as_bytes()
                                .get(1)
                                .map_or(false, |&b| b.is_ascii_alphabetic())
                        })
                        .map(|keyword| good_named_func(keyword))
                        .collect()
                } else {
                    self.registers
                        .keys()
                        .map(|keyword| good_named_func(keyword))
                        .collect()
                }
            }
            _ => {
                let good_named_func = |keyword: &str| {
                    info!("{}", keyword);
                    CompletionItem {
                        label: keyword.to_string(),
                        kind: Some(CompletionItemKind::KEYWORD),
                        detail: Some(
                            self.get_instruction_docs(self.instructions.get(keyword).unwrap()),
                        ),
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
                    }
                };

                // Completion is buggy when there is a dot.
                // This should fix this
                if let Some(dot_position) = line_content[..character as usize].rfind(".") {
                    let prefix = &line_content[starting_index as usize..character as usize];
                    self.instructions
                        .keys()
                        .filter_map(|i| {
                            if i.starts_with(prefix) {
                                info!("{} {} {}", character, dot_position, starting_index);
                                info!("{} : {}", prefix, &i[dot_position - starting_index as usize..]);
                                Some(good_named_func(
                                    &(prefix.to_owned() + &i[character as usize - starting_index as usize..]),
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

        if kind == "opcode" {
            if let Some(instruction) = self.instructions.get(cursor_node_text) {
                let documentation = self.get_instruction_docs(instruction);

                return Ok(Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(documentation)),
                    range: None,
                }));
            }
        } else if kind == "meta" {
            if let Some(directive) = self.directives.get(cursor_node_text) {
                return Ok(Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(String::from(directive))),
                    range: None,
                }));
            }
        } else if kind == "register" {
            if let Some(register) = self.registers.get(cursor_node_text) {
                return Ok(Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(String::from(register))),
                    range: None,
                }));
            }
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

        // Find all labels in file
        let labels = tree::find_labels(tree.root_node(), text.as_str());

        // Filter labels and match label below cursor
        for (label, start) in labels {
            if label == cursor_label_name {
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: text_document.uri,
                    range: Range {
                        start: tree::point_to_position(&start),
                        end: tree::point_to_position(&start),
                    },
                })));
            }
        }

        // Return None; no definition found
        Ok(None)
    }
}

mod json {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Instruction {
        pub syntax: String,
        pub description: String,
        pub format: String,
        pub code: String,
    }

    pub type Instructions = std::collections::HashMap<String, Vec<Instruction>>;
    pub type Directives = std::collections::HashMap<String, String>;
    pub type Registers = std::collections::HashMap<String, String>;

    pub fn read_instructions() -> Instructions {
        let json = include_str!("../resources/instructions.json");
        serde_json::from_str(json).expect("JSON parsing failed")
    }

    pub fn read_directives() -> Directives {
        let json = include_str!("../resources/directives.json");
        serde_json::from_str(json).expect("JSON parsing failed")
    }

    pub fn read_registers() -> Registers {
        let json = include_str!("../resources/registers.json");
        serde_json::from_str(json).expect("JSON parsing failed")
    }
}

mod tree {
    use tower_lsp::lsp_types::Position;
    use tracing::info;
    use tree_sitter::*;

    pub fn create_parser() -> Parser {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_mips::language()).unwrap();
        parser
    }

    pub fn byte_offset(text: &str, position: Position) -> usize {
        text.lines()
            .take(position.line as usize)
            .map(|line| line.len() + 1)
            .sum::<usize>()
            + position.character as usize
    }

    pub fn position_to_point(position: &Position) -> Point {
        Point {
            row: position.line as usize,
            column: position.character as usize,
        }
    }

    pub fn point_to_position(point: &Point) -> Position {
        Position {
            line: point.row as u32,
            character: point.column as u32,
        }
    }

    pub fn print_tree(node: tree_sitter::Node, indent: usize) {
        let mut cursor = node.walk();

        if node.kind() != "\n" {
            info!("|{}{:?}", "  ".repeat(indent), node.kind());
        }

        for child in node.children(&mut cursor) {
            if child.kind() == "ident" && node.kind() == "label" {}
            print_tree(child, indent + 2);
        }
    }

    pub fn find_labels(node: tree_sitter::Node, source_code: &str) -> Vec<(String, Point)> {
        let mut labels = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "ident" && node.kind() == "label" {
                let text = child.utf8_text(source_code.as_bytes()).unwrap_or_default();
                let start = child.start_position();
                labels.push((text.to_string(), start));
            }

            // Recursively check children
            labels.extend(find_labels(child, source_code));
        }
        labels
    }

    // Execute a query on a tree and return all matches
    //pub fn query<'a, T, I>(root: Node, content: &'a str, query_string: &str) -> QueryMatches<'a, 'a, T, I> {
    //    // Generate query
    //    let query =
    //        Query::new(&tree_sitter_mips::language(), query_string).expect("Error compiling query");
    //
    //    // Execute the query
    //    QueryCursor::new().matches(&query, root, content.as_bytes())
    //}
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
        let tree = &document.tree;
        let text = &document.text;

        parser::parse_meta_idents(&mut diagnostics, document.deref(), &self.directives);
        parser::parse_instructions(&mut diagnostics, document.deref(), &self.instructions);
        parser::parse_labels(&mut diagnostics, document.deref());

        diagnostics
    }

    fn get_instruction_docs(&self, instruction: &Vec<json::Instruction>) -> String {
        let mut docs = String::from("");
        for part in instruction {
            docs = format!(
                "{}```asm\n{}\n```\n{}\n{}\nInstruction format: {}\nMachine code: {}\n\n",
                docs,
                part.syntax,
                "─".repeat(part.syntax.len()),
                part.description,
                part.format,
                part.code
            );
        }
        docs
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

            let (service, socket) = LspService::new(|client| Backend::new(client));
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

mod parser {
    use std::str::Bytes;

    use crate::json::Directives;
    use crate::json::Instructions;
    use crate::tree;
    use crate::types;
    use tower_lsp::jsonrpc::Result;
    use tower_lsp::lsp_types::*;
    use tower_lsp::{Client, LanguageServer, LspService, Server};
    use tracing::info;
    use tree_sitter::{InputEdit, Language, Node, Parser, Query, QueryCursor, Tree};

    use streaming_iterator::StreamingIterator;

    fn add_diagnostic(
        diagnostics: &mut Vec<Diagnostic>,
        node: &Node,
        message: &str,
        severity: DiagnosticSeverity,
    ) {
        let start = tree::point_to_position(&node.start_position());
        let end = tree::point_to_position(&node.end_position());
        diagnostics.push(Diagnostic {
            range: Range { start, end },
            severity: Some(severity),
            code: None,
            code_description: None,
            source: Some("mips-language-server".to_string()),
            message: message.to_string(),
            related_information: None,
            tags: None,
            data: None,
        });
    }

    pub fn parse_meta_idents(
        diagnostics: &mut Vec<Diagnostic>,
        document: &types::Document,
        directives: &Directives,
    ) {
        let types::Document { tree, text } = document;

        // Generate query to find labels
        let query = Query::new(&tree_sitter_mips::language(), r#"(meta) @meta"#)
            .expect("Error compiling query");

        // Execute the query
        let mut query_cursor = QueryCursor::new();
        let mut matches = query_cursor.matches(&query, tree.root_node(), text.as_bytes());

        // Create hashset to collect data nodes
        let mut data_nodes = std::collections::HashSet::new();

        // Starting points of each section
        let mut sections = std::collections::HashMap::new();
        //let mut data_start = None;
        //let mut text_start = None;
        //let mut kdata_start = None;
        //let mut ktext_start = None;

        // Iterate over the matches
        //for m in matches {
        while let Some(m) = matches.next() {
            if let Some(capture) = m.captures.first() {
                let node = capture.node;
                let meta_text = &text[node.start_byte()..node.end_byte()];
                let start = tree::point_to_position(&node.start_position());
                let end = tree::point_to_position(&node.end_position());

                if !directives.contains_key(meta_text) {
                    add_diagnostic(
                        diagnostics,
                        &node,
                        "Unregistered directive",
                        DiagnosticSeverity::ERROR,
                    );
                    continue;
                }

                match meta_text {
                    ".data" | ".kdata" | ".sdata" | ".rdata" | ".text" | ".ktext" | ".bss"
                    | ".sbss" => {
                        if sections.contains_key(meta_text) {
                            add_diagnostic(
                                diagnostics,
                                &node,
                                format!("Multiple {} sections", meta_text).as_str(),
                                DiagnosticSeverity::ERROR,
                            );
                            continue;
                        }
                        sections.insert(meta_text, start.line);
                    }
                    ".word" | ".ascii" | ".asciiz" | ".byte" | ".align" | ".half" | ".space"
                    | ".double" | ".float" => {
                        data_nodes.insert(node);
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn parse_instructions(
        diagnostics: &mut Vec<Diagnostic>,
        document: &types::Document,
        instructions: &Instructions,
    ) {
        let types::Document { tree, text } = document;

        // Generate query to find labels
        let query = Query::new(
            &tree_sitter_mips::language(),
            r#"(instruction) @instruction"#,
        )
        .expect("Error compiling query");

        // Execute the query
        let mut query_cursor = QueryCursor::new();
        let mut matches = query_cursor.matches(&query, tree.root_node(), text.as_bytes());

        // Iterate over the matches
        //for m in matches {
        while let Some(m) = matches.next() {
            if let Some(capture) = m.captures.first() {
                let node = capture.node;
                //let instruction_name = &text[node.start_byte()..node.end_byte()];
                let mut cursor = node.walk();
                let mut instruction_name;

                for child in node.children(&mut cursor) {
                    if child.kind() == "opcode" {
                        instruction_name = &text[child.start_byte()..child.end_byte()];
                        info!("inst:{}!", instruction_name);
                        if !instructions.contains_key(instruction_name) {
                            let start = tree::point_to_position(&node.start_position());
                            let end = tree::point_to_position(&node.end_position());
                            add_diagnostic(
                                diagnostics,
                                &node,
                                "Unknown instruction",
                                DiagnosticSeverity::ERROR,
                            );
                        }
                    }
                }
            }
        }
    }

    pub fn parse_labels(diagnostics: &mut Vec<Diagnostic>, document: &types::Document) {
        let types::Document { tree, text } = document;

        // Generate query to find labels
        let query = Query::new(&tree_sitter_mips::language(), r#"(label) @label"#)
            .expect("Error compiling query");

        // Execute the query
        let mut query_cursor = QueryCursor::new();
        let mut matches = query_cursor.matches(&query, tree.root_node(), text.as_bytes());

        // Create hashset to collect label names
        let mut label_texts = std::collections::HashSet::new();

        // Iterate over the matches
        //for m in matches {
        while let Some(m) = matches.next() {
            if let Some(capture) = m.captures.first() {
                let node = capture.node;
                let label_text = &text[node.start_byte()..node.end_byte()];

                if label_texts.contains(label_text) {
                    info!("Duplicate label! {}", label_text);
                    let start = tree::point_to_position(&node.start_position());
                    let end = tree::point_to_position(&node.end_position());
                    add_diagnostic(
                        diagnostics,
                        &node,
                        "Duplicate jump label",
                        DiagnosticSeverity::ERROR,
                    );
                } else {
                    label_texts.insert(label_text);
                }
            }
        }
    }

    // Parse an argument and return it's type
    // Examples:
    //  [register]: $t0, $s1, $a0, $v0
    //  [float register]: $f0, $f1, $f31, $f12
    //  [invalid register]: $abc
    //  [immediate]: 24, 2134, 0x3fa, 0274124, 94967296
    //  [address]: x, y, x($t0), 4($t0), 0xa8($s1)
    //  [jump label]: main, loop
    fn parse_argument(argument: &str) -> Option<&str> {
        if argument.len() == 0 {
            return None;
        }

        let registers = [
            "$0", "$1", "$2", "$3", "$4", "$5", "$6", "$7", "$8", "$9", "$10", "$11", "$12", "$13",
            "$14", "$15", "$16", "$17", "$18", "$19", "$20", "$21", "$22", "$23", "$24", "$25",
            "$26", "$27", "$28", "$29", "$30", "$31", "$zero", "$at", "$v0", "$v1", "$a0", "$a1",
            "$a2", "$a3", "$t0", "$t1", "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t8", "$t9",
            "$s0", "$s1", "$s2", "$s3", "$s4", "$s5", "$s6", "$s7", "$k0", "$k1", "$gp", "$sp",
            "$fp", "$ra",
        ];
        let float_registers = [
            "$f0", "$f1", "$f2", "$f3", "$f4", "$f5", "$f6", "$f7", "$f8", "$f9", "$f10", "$f11",
            "$f12", "$f13", "$f14", "$f15", "$f16", "$f17", "$f18", "$f19", "$f20", "$f21", "$f22",
            "$f23", "$f24", "$f25", "$f26", "$f27", "$f28", "$f29", "$f30", "$f31",
        ];

        // Check if the argument is a register
        if argument.starts_with("$") {
            if registers.contains(&argument) {
                return Some("register");
            }
            if float_registers.contains(&argument) {
                return Some("float register");
            }
            return Some("invalid register");
        }

        // Assume argument is decimal immediate or address
        let mut rmap: fn(char) -> bool;
        rmap = |x| x.is_ascii_digit();
        let mut rtype = "immediate";

        // Ignore sign prefix
        let mut offset = if argument.starts_with("-") { 1 } else { 0 };

        let bytes = argument.as_bytes();
        if bytes[0].is_ascii_alphabetic() {
            // Argument is label or address (if valid)
            rmap = |byte| byte.is_ascii_alphanumeric();
            rtype = "label";
        } else if argument.starts_with("0x") {
            // Argument is immediate or address (if valid)
            rmap = |byte| byte.is_ascii_hexdigit();
            rtype = "immediate";
            offset += 2;
        }

        let mut bracket = false;
        let mut sub_start = 0;

        for (i, char) in argument.chars().enumerate() {
            if i < offset || rmap(char) {
                continue;
            }

            if char == '(' {
                if bracket || sub_start != 0 {
                    // When a bracket was already found
                    return Some("invalid address");
                }
                bracket = true;
                sub_start = i + 1;
            }

            if char == ')' {
                if i + 1 != argument.len() || !bracket {
                    // When no opening bracket was found or symbols follow closing bracket
                    return Some("invalid address");
                }
                bracket = false;
            }
        }

        // If brackets were found, an address is expected
        if sub_start > 0 {
            if bracket {
                // missing closing bracket
                return Some("address missing bracket");
            }

            let sub_string = &argument[sub_start..argument.len() - 1];
            if registers.contains(&sub_string) {
                return Some("address");
            }

            // Substring in brackets is not a register
            return Some("invalid address");
        }

        // Valid argument, return type
        return Some(rtype);
    }

    fn examples(string: &str) -> String {
        string
            .replacen("[register]", "$t1", 1)
            .replacen("[register]", "$t2", 1)
            .replacen("[register]", "$t3", 1)
            .replacen("[float register]", "$f0", 1)
            .replacen("[float register]", "$f2", 1)
            .replacen("[float register]", "$f4", 1)
            .replacen("[coprocessor register]", "$8", 1)
            .replacen("[address]", "4($t1)", 1)
            .replacen("[jump label]", "target", 1)
    }
}
