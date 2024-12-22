use dashmap::DashMap;
use std::io::BufWriter;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tracing::info;
use tracing::level_filters::LevelFilter;
use ts::find_labels;

#[derive(Debug)]
struct Backend {
    client: Client,
    documents: DashMap<Url, String>,
    //documents: Mutex<HashMap<String, String>>, // Full text of each document by URI
    //labels: Mutex<HashMap<String, Vec<(String, usize)>>>, // Jump labels by URI: (label, line number)
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
        let TextDocumentItem { uri, text, .. } = params.text_document;

        info!(
            "textDocument/didOpen: {:?}",
            uri.to_file_path().expect("expected file")
        );
        info!("Content:\n{}", &text);

        self.documents.insert(uri, text);
        info!("docs: {:?}", &self.documents);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        info!("textDocument/didChange");

        let changes = params.content_changes;
        for change in &changes {
            info!("Change:\n{}", &change.text);
        }

        if changes.len() != 1 {
            panic!("You cannot do more than one {:?}", changes);
        }
        let text = changes[0].text.clone();
        self.documents.insert(params.text_document.uri, text);
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
        Ok(Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String("You're hovering!".to_string())),
            range: None,
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let TextDocumentPositionParams {
            position,
            text_document,
        } = params.text_document_position_params;

        let tmp = self
            .documents
            .get(&text_document.uri)
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())?;
        info!("text: {:?}", tmp.as_str());

        let text = self
            .documents
            .get(&text_document.uri)
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())?;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_asm::language())
            .expect("Error loading asm grammar");

        let tree = parser
            .parse(text.as_bytes(), None)
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())?;

        let point = ts::position_to_point(&position);
        let node = ts::get_node_at_point(&tree, point)
            .ok_or(tower_lsp::jsonrpc::Error::invalid_request())?;

        let text = node.utf8_text(tmp.as_bytes()).unwrap_or_default();

        let root = tree.root_node();
        let labels = find_labels(root, tmp.as_str());

        for (label, start) in labels {
            if label == text {
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: text_document.uri,
                    range: Range {
                        start: ts::point_to_position(&start),
                        end: ts::point_to_position(&start),
                    },
                })));

            }
        }


        //let Position { line, character } = position;

        //let start = node.start_position();


        Ok(None)
    }
}

mod ts {
    use std::usize;

    use tower_lsp::lsp_types::Position;
    use tracing::info;
    use tree_sitter::{Node, Point};

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

    pub fn get_node_at_point(tree: &tree_sitter::Tree, point: Point) -> Option<Node> {
        tree.root_node().descendant_for_point_range(point, point)
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
            documents: DashMap::new(),
            //documents: Mutex::new(HashMap::new()),
            //labels: Mutex::new(HashMap::new()),
        }
    }

    // Parse a document and extract jump labels
    //fn parse_document(&self, uri: &str, content: &str) {
    //    let mut labels = Vec::new();
    //    for (line_number, line) in content.lines().enumerate() {
    //        if let Some(label) = Self::extract_label(line) {
    //            labels.push((label, line_number));
    //        }
    //    }
    //    self.labels.lock().unwrap().insert(uri.to_string(), labels);
    //}
    //
    ///// Extract a jump label from a single line (if any)
    //fn extract_label(line: &str) -> Option<String> {
    //    // Assume labels end with ':'
    //    if let Some(index) = line.find(':') {
    //        let label = &line[..index];
    //        if !label.trim().is_empty() {
    //            return Some(label.trim().to_string());
    //        }
    //    }
    //    None
    //}

    //async fn on_change<'a>(&self, params: TextDocumentItem<'a>) {
    //    dbg!(&params.version);
    //    let rope = ropey::Rope::from_str(params.text);
    //    self.document_map
    //        .insert(params.uri.to_string(), rope.clone());
    //}
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args();
    match args.nth(1).as_deref() {
        None => {
            let log_file = std::fs::File::create("/home/oskar/git/mips-language-server/lsp.log")
                .expect("Create file");
            let log_file = BufWriter::new(log_file);
            let (non_blocking, _guard) = tracing_appender::non_blocking(log_file);
            let subscriber = tracing_subscriber::fmt()
                .with_max_level(LevelFilter::DEBUG)
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
