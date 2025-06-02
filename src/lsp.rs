#[derive(Debug)]
pub struct Document {
    pub text: String,
    pub tree: tree_sitter::Tree,
}

pub type Documents = dashmap::DashMap<tower_lsp::lsp_types::Url, Document>;
