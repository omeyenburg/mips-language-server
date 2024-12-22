//use std::collections::HashMap;
use dashmap::DashMap;
use std::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

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
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        //debug!("file opened");
        //self.on_change(TextDocumentItem {
        //    uri: params.text_document.uri,
        //    text: &params.text_document.text,
        //    version: Some(params.text_document.version),
        //})
        //.await
        self.client
            .log_message(MessageType::INFO, "file opened")
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let changes = params.content_changes;
        if changes.len() != 1 {
            panic!("You cannot do more than one {:?}", changes);
        }
        let text = changes[0].text.clone();
        self.documents.insert(params.text_document.uri, text);

        self.client
            .log_message(MessageType::INFO, "file changed")
            .await;

        //self.on_change(TextDocumentItem {
        //    text: &params.content_changes[0].text,
        //    uri: params.text_document.uri,
        //    version: Some(params.text_document.version),
        //})
        //.await
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

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
        Ok(Some(GotoDefinitionResponse::Scalar(Location {
            uri: params.text_document_position_params.text_document.uri,
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 0,
                },
            },
        })))
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
