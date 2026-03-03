pub mod utils;

use tower_lsp_server::{
    jsonrpc,
    ls_types::{Diagnostic, Uri},
};
use tree_sitter::*;

use crate::{
    ast::{self, parser, Ast},
    document,
    semantic::{self, SemanticModel},
};

pub struct Document {
    pub uri: Uri,
    pub version: i32,
    pub text: String,
    pub tree: Tree,
    pub parser: Parser,
    pub ast: Ast,
    pub semantic_model: SemanticModel,
}

impl Document {
    pub fn new(uri: Uri, version: i32, text: String) -> Document {
        let semantic_model = SemanticModel::new();
        let ast = Ast::new();
        let mut parser = document::utils::create_parser();

        // Generate tree using tree-sitter
        let tree = parser
            .parse(text.as_bytes(), None)
            .ok_or(jsonrpc::Error::invalid_request())
            .expect("Expected new tree");

        Document {
            version,
            text,
            tree,
            parser,
            uri,
            ast,
            semantic_model,
        }
    }

    pub fn apply_change(&mut self, change_text: &str, range: Range) {
        // Stop the server from crashing here
        // Weird bug, maybe happens with multiple or discarded changes
        if range.end_byte >= self.text.len() {
            log!("Error: something wrong with the bytes but idk - please write an issue on https://github.com/omeyenburg/mips-language-server");
            return;
        }

        // Calculate new text, old text and apply change to text
        self.text
            .replace_range(range.start_byte..range.end_byte, change_text);

        // Create an InputEdit
        let input_edit = InputEdit {
            start_byte: range.start_byte,
            old_end_byte: range.end_byte,
            new_end_byte: range.start_byte + self.text.len(),
            start_position: range.start_point,
            old_end_position: range.end_point,
            new_end_position: Point {
                row: range.start_point.row + change_text.lines().count(), // - 1, // stupid to substract 1. why?!
                column: if let Some(last_line) = change_text.lines().last() {
                    last_line.len()
                } else {
                    range.start_point.column
                },
            },
        };

        // Apply InputEdit to the tree
        self.tree.edit(&input_edit);

        // Update the tree incrementally
        let new_tree = self.parser.parse(&self.text, Some(&self.tree)).unwrap();
        self.tree = new_tree;
    }

    pub fn parse_entire_document(&mut self, change_text: &str) {
        self.text = change_text.to_string();
        self.tree = self.parser.parse(&self.text, None).unwrap();
    }

    pub async fn analyze(&mut self) -> Vec<Diagnostic> {
        self.ast = ast::Ast::from_ts_tree(&self.text, &self.tree);

        self.semantic_model = semantic::SemanticModel::new();
        self.semantic_model.parse(&self.text, &self.ast);

        // Analyze document and publish diagnostics
        // let diags = self.analyze_document(&uri).await;
        self.analyze_document().await
    }
}
