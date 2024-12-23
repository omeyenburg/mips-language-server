use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tracing::info;

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
                completion_provider: Some(CompletionOptions::default()),
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
        let tree = ts::create_tree(text.as_bytes(), None)
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())
            .expect("Expected new tree");

        info!("Text: {}", &text);

        // Store document text and tree
        self.documents.insert(uri, Document { text, tree });
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        info!("textDocument/didChange");

        let uri = params.text_document.uri.clone();
        let changes = params.content_changes;

        // Expect only one change at once
        if changes.len() != 1 {
            panic!("You cannot do more than one {:?}", changes);
        }
        let text = changes[0].text.clone();

        // TODO: here we could check, if range.is_none() is true,
        // then the editor does not properly support incremental
        // parsing.
        // my current setup seems to not allow incremental
        // parsing, so i leave it out for now
        // info!("Changes: {:?}", &changes);

        // Retrieve old tree
        //let old_tree = &self
        //    .documents
        //    .get(&uri)
        //    .ok_or(tower_lsp::jsonrpc::Error::invalid_request())
        //    .expect("Expected old tree")
        //    .tree
        //    .clone();

        // Generate new tree using tree-sitter
        // We pass None here as the old tree.
        // We would have to pass the old tree from above for incremental
        // parsing. But for some reason, the tree does not update well,
        // if we append or remove lines at the end of a file.
        //let tree = ts::create_tree(text.as_bytes(), Some(&old_tree))
        let tree = ts::create_tree(text.as_bytes(), None)
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())
            .expect("Expected new tree");

        // Update document text and tree
        self.documents.insert(uri.clone(), Document { text, tree });

        // Debugging output
        //let document = self
        //    .documents
        //    .get(&uri)
        //    .ok_or(tower_lsp::jsonrpc::Error::invalid_request())
        //    .expect("Expected document");
        //let text = &document.text;
        //let tree = document.tree.clone();
        //
        //let root = tree.root_node();
        //info!("Updated text and tree:\n{}", text);
        //ts::print_tree(root, 0);
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        //let uri = params.text_document_position.text_document.uri;
        //let position = params.text_document_position.position;

        // Define a list of MIPS assembly instructions and keywords
        let keywords = vec![
            "add", "sub", "mul", "div", "and", "or", "xor", "sll", "srl", "lw", "sw", "beq", "bne",
            "j", "jal", "jr", "la", "li", "move", "neg", "not", "slt", "slti", "mul", "divu",
            "mfhi", "mflo", "mthi", "mtlo",
        ];

        //let line = get_line_from_document(&params.text_document_position).await;
        //let line = get_line_from_document(self, &params.text_document_position)
        //    .await
        //    .unwrap_or_default();
        //let labels = get_labels_from_document(
        //    self,
        //    &params.text_document_position.text_document.uri.to_string(),
        //)
        //.await;

        // Generate completion items
        let items: Vec<CompletionItem> = keywords
            .iter()
            .map(|&keyword| CompletionItem {
                label: keyword.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("MIPS instruction".to_string()),
                ..Default::default()
            })
            .collect();

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

        let root = tree.root_node();
        ts::print_tree(root, 0);

        // Determine node below cursor and fetch the label name
        let point = ts::position_to_point(&position);
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
                for (i, example) in instruction.iter().enumerate() {
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
            let documentation;
            if cursor_node_text == ".data" {
                documentation = String::from("Data section of the program");
            } else if cursor_node_text == ".text" {
                documentation = String::from("Code section of the program");
            } else {
                return Ok(None);
            }

            return Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(documentation)),
                range: None,
            }));
        } else if kind == "address" {
            return Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(String::from("This is an address"))),
                range: None,
            }));
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
        //let tree = document.tree; //.clone();

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_asm::language())
            .expect("Error loading asm grammar");

        let tree = parser
            .parse(text.as_bytes(), Some(&document.tree))
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())
            .expect("Expected new tree")
            .clone();

        let root = document.tree.root_node();
        ts::print_tree(root, 0);

        // Determine node below cursor and fetch the label name
        let point = ts::position_to_point(&position);
        let cursor_label_name = tree
            .root_node()
            .descendant_for_point_range(point, point)
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())?
            .utf8_text(text.as_bytes())
            .unwrap_or_default();

        info!(
            "p: {:?}, {:?}, {:?}, {}",
            position,
            point,
            tree.root_node().descendant_for_point_range(point, point),
            cursor_label_name
        );
        info!("{}", &text);

        // Find all labels in file
        let labels = ts::find_labels(tree.root_node(), text.as_str());

        // Filter labels and match label below cursor
        for (label, start) in labels {
            if label == cursor_label_name {
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: text_document.uri,
                    range: Range {
                        start: ts::point_to_position(&start),
                        end: ts::point_to_position(&start),
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

    pub fn read() -> Instructions {
        let json = include_str!("../resources/instructions.json");
        serde_json::from_str(json).expect("JSON parsing failed")
    }
}

mod ts {
    use tower_lsp::lsp_types::Position;
    use tracing::info;
    use tree_sitter::{Node, Point, Tree};

    pub fn create_tree(text: &[u8], old_tree: Option<&Tree>) -> Option<Tree> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_asm::language())
            .expect("Error loading asm grammar");

        parser.parse(text, old_tree)
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

        //info!("{}Node: {:?}, {:?}", "  ".repeat(indent), node, node.kind());
        if node.kind() != "\n" {
            info!("{}{:?}", "  ".repeat(indent), node.kind());
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
        let instructions = json::read();

        Self {
            client,
            documents,
            instructions,
        }
    }
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args();
    match args.nth(1).as_deref() {
        None => {
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
