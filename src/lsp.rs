use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Document {
    pub version: i32,
    pub text: String,
    pub tree: tree_sitter::Tree,
    pub parser: tree_sitter::Parser,
}

pub type Documents = dashmap::DashMap<tower_lsp_server::lsp_types::Uri, Arc<RwLock<Document>>>;
