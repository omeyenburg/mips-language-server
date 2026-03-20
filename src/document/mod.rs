pub mod utf16;
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
            new_end_byte: range.start_byte + change_text.len(),
            start_position: range.start_point,
            old_end_position: range.end_point,
            new_end_position: Point {
                row: range.start_point.row + change_text.lines().count(),
                column: if let Some(last_line) = change_text.lines().last() {
                    last_line.encode_utf16().count()
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

    pub fn position_to_point(&self, position: &tower_lsp_server::ls_types::Position) -> Point {
        let byte = self.position_to_byte(position);
        self.byte_to_point(byte)
    }

    pub fn position_to_byte(&self, position: &tower_lsp_server::ls_types::Position) -> usize {
        let line_num = position.line as usize;
        let line = self.text.lines().nth(line_num).unwrap_or("");

        let bytes_before: usize = self.text.lines().take(line_num).map(|l| l.len() + 1).sum();
        let char_idx = utf16::utf16_to_char_index(line, position.character);
        let line_bytes: usize = line.chars().take(char_idx).map(|c| c.len_utf8()).sum();

        bytes_before + line_bytes
    }

    pub fn point_to_position(&self, point: &Point) -> tower_lsp_server::ls_types::Position {
        let line_bytes: usize = self.text.lines().take(point.row).map(|l| l.len() + 1).sum();
        self.byte_to_position(line_bytes + point.column)
    }

    pub fn byte_to_point(&self, byte_offset: usize) -> Point {
        let mut row = 0;
        let mut col = 0;
        let mut current = 0;

        for ch in self.text.chars() {
            if current >= byte_offset {
                break;
            }
            if ch == '\n' {
                row += 1;
                col = 0;
            } else {
                col += ch.len_utf8();
            }
            current += ch.len_utf8();
        }

        Point { row, column: col }
    }

    pub fn byte_to_position(&self, byte_offset: usize) -> tower_lsp_server::ls_types::Position {
        let mut line = 0;
        let mut byte_count = 0;
        let mut line_start = 0;

        for (i, ch) in self.text.char_indices() {
            if byte_count >= byte_offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                line_start = i + 1;
            }
            byte_count += ch.len_utf8();
        }

        let line_text = self.text.lines().nth(line as usize).unwrap_or("");
        let offset_in_line = byte_offset - line_start;

        let mut bytes_in_line = 0;
        let mut utf16_count = 0;

        for ch in line_text.chars() {
            if bytes_in_line >= offset_in_line {
                break;
            }
            bytes_in_line += ch.len_utf8();
            utf16_count += ch.len_utf16() as u32;
        }

        tower_lsp_server::ls_types::Position {
            line,
            character: utf16_count,
        }
    }

    pub fn ls_range_to_ts(&self, range: &tower_lsp_server::ls_types::Range) -> Range {
        Range {
            start_byte: self.position_to_byte(&range.start),
            end_byte: self.position_to_byte(&range.end),
            start_point: self.position_to_point(&range.start),
            end_point: self.position_to_point(&range.end),
        }
    }
}
