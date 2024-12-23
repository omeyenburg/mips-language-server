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
    //documents: dashmap::DashMap<Url, String>,
    //trees: dashmap::DashMap<Url, tree_sitter::Tree>,
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

        let root = tree.root_node();
        ts::print_tree(root, 0);

        // Store document text and tree
        self.documents.insert(uri, Document { text, tree });
        //self.documents.insert(uri.clone(), text);
        //self.trees.insert(uri, tree.clone());
        //info!("inserted")
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        info!("textDocument/didChange");

        let uri = params.text_document.uri.clone();
        let changes = params.content_changes;

        // Expect only one change
        if changes.len() != 1 {
            panic!("You cannot do more than one {:?}", changes);
        }
        let text = changes[0].text.clone();

        info!(" - text generated");

        let old_tree = self
            .documents
            .get(&uri)
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())
            .expect("Expected old tree")
            .tree
            .clone();

        // Retrieve old tree
        //let old_tree = self
        //    .trees
        //    .get(&uri)
        //    .ok_or(tower_lsp::jsonrpc::Error::invalid_request())
        //    .expect("Expected old tree");

        info!(" - tree found");

        // Generate tree using tree-sitter
        let tree = ts::create_tree(text.as_bytes(), Some(&old_tree))
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())
            .expect("Expected new tree");

        info!(" - tree generated");
        let root = tree.root_node();
        ts::print_tree(root, 0);

        // Update document text and tree
        //info!("Inserting tree for URI: {:?}", params.text_document.uri);
        //self.trees
        //    .insert(params.text_document.uri.clone(), tree.clone());
        //info!(" - first insert");
        //self.documents
        //    .insert(params.text_document.uri.clone(), text);
        self.documents.insert(uri, Document { text, tree });
        info!(" - insert successful");
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

    async fn hover(&self, _: HoverParams) -> Result<Option<Hover>> {
        /*
        Documentation should look like this:


                */

        Ok(Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String("You're hovering!".to_string())),
            range: None,
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
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
        let point = ts::position_to_point(&position);
        let cursor_label_name = tree
            .root_node()
            .descendant_for_point_range(point, point)
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())?
            .utf8_text(text.as_bytes())
            .unwrap_or_default();

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

        info!("{}Node: {:?}, {:?}", "  ".repeat(indent), node, node.kind());

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
        Self {
            client,
            documents: dashmap::DashMap::new(),
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
