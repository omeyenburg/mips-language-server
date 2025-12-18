#[derive(Debug)]
pub struct Document {
    pub text: String,
    pub tree: tree_sitter::Tree,
}

pub type Documents = dashmap::DashMap<tower_lsp_server::lsp_types::Uri, Document>;
