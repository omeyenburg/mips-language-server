use std::collections::HashMap;

use serde::de::value;
use tower_lsp_server::jsonrpc;
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use tree_sitter::{InputEdit, Query, QueryCursor};

use crate::lang::LanguageDefinitions;
use crate::lang::{Directive, Instruction, Registers};
use crate::server::{Backend, Document, Documents};

impl Backend {
    pub async fn analyze_document(&self, uri: &Uri) -> Vec<Diagnostic> {
        // let doc = self.documents.get(uri).unwrap().read().unwrap();
        vec![]
    }
}
