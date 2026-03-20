use tower_lsp_server::ls_types;
use tree_sitter::Parser;

pub fn create_parser() -> Parser {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_mips::LANGUAGE.into())
        .unwrap();
    parser
}

pub fn ascii_ts_range_to_ls(range: &tree_sitter::Range) -> ls_types::Range {
    ls_types::Range {
        start: ls_types::Position {
            line: range.start_point.row as u32,
            character: range.start_point.column as u32,
        },
        end: ls_types::Position {
            line: range.end_point.row as u32,
            character: range.end_point.column as u32,
        },
    }
}

pub fn get_text_in_ts_range(text: &str, range: tree_sitter::Range) -> &str {
    &text[range.start_byte..range.end_byte]
}
