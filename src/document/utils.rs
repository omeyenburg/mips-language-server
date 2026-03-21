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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_line_starts_empty() {
        assert_eq!(calculate_line_starts(""), vec![0]);
    }

    #[test]
    fn test_calculate_line_starts_single_line() {
        assert_eq!(calculate_line_starts("hello world"), vec![0]);
    }

    #[test]
    fn test_calculate_line_starts_multiple_lines() {
        let text = "line1\nline2\nline3";
        assert_eq!(calculate_line_starts(text), vec![0, 6, 12]);
    }

    #[test]
    fn test_calculate_line_starts_with_emoji() {
        let text = "# 😀 test\nadd $t0";
        // 😀 is 4 bytes, so line 2 starts at byte 12
        assert_eq!(calculate_line_starts(text), vec![0, 12]);
    }
}
