#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tracing::info;
use tree_sitter::{InputEdit, Parser, Tree};

#[derive(Debug)]
struct Document {
    text: String,
    tree: tree_sitter::Tree,
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
        self.documents.insert(uri, Document { text, tree });

        // Parse the document
        self.parse();
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

        // Parse the document
        self.parse();
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let mut keywords = Vec::new();
        keywords.extend(self.instructions.keys().cloned());
        keywords.extend(self.directives.keys().cloned());
        keywords.extend(self.registers.keys().cloned());

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
                    detail: Some("MIPS directive".to_string()),
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
            '$' => self
                .registers
                .keys()
                .map(|keyword| CompletionItem {
                    label: keyword.to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    detail: Some("MIPS register".to_string()),
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
            _ => self
                .instructions
                .keys()
                .map(|keyword| CompletionItem {
                    label: keyword.to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    detail: Some("MIPS instruction".to_string()),
                    ..Default::default()
                })
                .collect(),
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

        if kind == "word" {
            if let Some(instruction) = self.instructions.get(cursor_node_text) {
                let mut documentation = String::from("");
                for example in instruction {
                    documentation = documentation
                        + format!(
                            "```asm\n{}\n```\n{}\n {}\n Instruction format: {}\n Machine code: {}\n\n",
                            example.syntax, "─".repeat(example.syntax.len()), example.description, example.format, example.code
                        ).as_str();
                }

                return Ok(Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(documentation)),
                    range: None,
                }));
            }
        } else if kind == "meta_ident" {
            if let Some(directive) = self.directives.get(cursor_node_text) {
                return Ok(Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(String::from(directive))),
                    range: None,
                }));
            }
        } else if kind == "address" {
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
    use tree_sitter::{Parser, Point, Tree};

    pub fn create_parser() -> Parser {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_asm::language()).unwrap();
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

    fn parse(&self) {
        /* Parse the document (for now not incrementally)

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
