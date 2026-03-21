pub mod utf16;
pub mod utils;

use tower_lsp_server::{
    jsonrpc,
    ls_types::{Diagnostic, Uri},
};
use tree_sitter::*;

use crate::{ast::Ast, document, semantic::SemanticModel};

pub struct Document {
    /// resource identifier of document
    pub uri: Uri,
    /// version of document set by client
    pub version: i32,
    /// full document text
    pub text: String,
    /// line to byte offset in text conversion (calculated from text)
    pub line_starts: Vec<usize>,
    /// concrete tree-sitter syntax tree
    pub tree: Tree,
    /// tree-sitter parsr
    pub parser: Parser,
    /// abstract syntax tree
    pub ast: Ast,
    /// semantic document information
    pub semantic_model: SemanticModel,
}

impl Document {
    pub fn new(uri: Uri, version: i32, text: String) -> Document {
        let semantic_model = SemanticModel::new();
        let ast = Ast::new();
        let mut parser = utils::create_parser();

        // Generate tree using tree-sitter
        let tree = parser
            .parse(text.as_bytes(), None)
            .ok_or(jsonrpc::Error::invalid_request())
            .expect("Expected new tree");

        // Calculate line_starts
        let line_starts = document::utils::calculate_line_starts(&text);

        Document {
            version,
            text,
            line_starts,
            tree,
            parser,
            uri,
            ast,
            semantic_model,
        }
    }

    pub fn apply_change(&mut self, change_text: &str, range: Range) {
        // Bounds check: if out of range, skip this change (sync issue between client and server)
        if range.start_byte > self.text.len() || range.end_byte > self.text.len() {
            log!("Warning: byte range out of bounds (start: {}, end: {}, text len: {}). Skipping change.", range.start_byte, range.end_byte, self.text.len());
            // Don't apply the change - wait for a full sync
            return;
        }

        if range.end_byte < range.start_byte {
            log!("Warning: invalid range (start > end). Skipping change.");
            return;
        }

        // Calculate new text, old text and apply change to text
        self.text
            .replace_range(range.start_byte..range.end_byte, change_text);

        // Recalculate line_starts after text change
        self.line_starts = document::utils::calculate_line_starts(&self.text);

        // Create an InputEdit
        let input_edit = InputEdit {
            start_byte: range.start_byte,
            old_end_byte: range.end_byte,
            new_end_byte: range.start_byte + change_text.len(),
            start_position: range.start_point,
            old_end_position: range.end_point,
            new_end_position: Point {
                row: range.start_point.row + change_text.matches('\n').count(),
                column: if let Some(last_line) = change_text.rsplit('\n').next() {
                    if change_text.contains('\n') {
                        last_line.len()
                    } else {
                        range.start_point.column + last_line.len()
                    }
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

    pub fn parse_entire_document(&mut self, new_text: &str) {
        self.text = new_text.to_string();
        self.line_starts = document::utils::calculate_line_starts(&self.text);
        self.tree = self.parser.parse(&self.text, None).unwrap();
    }

    pub async fn analyze(&mut self) -> Vec<Diagnostic> {
        self.ast = Ast::from_ts_tree(&self.text, &self.tree);

        self.semantic_model = SemanticModel::new();
        self.semantic_model.parse(&self.text, &self.ast);

        // Analyze document and publish diagnostics
        self.analyze_document().await
    }

    pub fn position_to_point(&self, position: &tower_lsp_server::ls_types::Position) -> Point {
        let byte = self.position_to_byte(position);
        self.byte_to_point(byte)
    }

    pub fn position_to_byte(&self, position: &tower_lsp_server::ls_types::Position) -> usize {
        let line_num = position.line as usize;

        let line_start = self.line_starts[line_num];
        let line_end = self
            .line_starts
            .get(line_num + 1)
            .copied()
            .unwrap_or(self.text.len());

        let line_content = &self.text[line_start..line_end];

        let char_idx = utf16::utf16_to_char_index(line_content, position.character);

        let byte_offset = line_content
            .chars()
            .take(char_idx)
            .map(|c| c.len_utf8())
            .sum::<usize>();

        line_start + byte_offset
    }

    pub fn point_to_position(&self, point: &Point) -> tower_lsp_server::ls_types::Position {
        let line_start = self.line_starts.get(point.row).copied().unwrap_or(0);
        self.byte_to_position(line_start + point.column)
    }

    // TODO: optimize with line_starts
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

    // TODO: optimize with line_starts
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

    pub fn ts_range_to_ls(&self, range: &tree_sitter::Range) -> tower_lsp_server::ls_types::Range {
        tower_lsp_server::ls_types::Range {
            start: self.point_to_position(&range.start_point),
            end: self.point_to_position(&range.end_point),
        }
    }
}
