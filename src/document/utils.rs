use tower_lsp_server::ls_types;
use tree_sitter::Parser;

pub fn create_parser() -> Parser {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_mips::LANGUAGE.into())
        .unwrap();
    parser
}

pub fn get_text_in_ts_range(text: &str, range: tree_sitter::Range) -> &str {
    &text[range.start_byte..range.end_byte]
}

pub fn calculate_line_starts(text: &str) -> Vec<usize> {
    let mut line_starts = vec![0];
    for (idx, ch) in text.char_indices() {
        if ch == '\n' {
            line_starts.push(idx + 1);
        }
    }
    line_starts
}
