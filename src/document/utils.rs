use tower_lsp_server::ls_types;
use tree_sitter::Parser;

pub fn create_parser() -> Parser {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_mips::LANGUAGE.into())
        .unwrap();
    parser
}

pub fn ls_range_to_ts_range(text: &str, range: &ls_types::Range) -> tree_sitter::Range {
    tree_sitter::Range {
        start_byte: ls_position_to_byte_offset(text, &range.start),
        end_byte: ls_position_to_byte_offset(text, &range.end),
        start_point: ls_position_to_ts_point(&range.start),
        end_point: ls_position_to_ts_point(&range.end),
    }
}

pub fn ts_range_to_ls_range(range: &tree_sitter::Range) -> ls_types::Range {
    ls_types::Range {
        start: ts_point_to_ls_position(&range.start_point),
        end: ts_point_to_ls_position(&range.end_point),
    }
}

pub fn ls_position_to_byte_offset(text: &str, position: &ls_types::Position) -> usize {
    text.lines()
        .take(position.line as usize)
        .map(|line| line.len() + 1)
        .sum::<usize>()
        + position.character as usize
}

pub fn ls_position_to_ts_point(position: &ls_types::Position) -> tree_sitter::Point {
    tree_sitter::Point {
        row: position.line as usize,
        column: position.character as usize,
    }
}

pub fn ts_point_to_ls_position(point: &tree_sitter::Point) -> ls_types::Position {
    ls_types::Position {
        line: point.row as u32,
        character: point.column as u32,
    }
}

pub fn get_text_in_ts_range(text: &str, range: tree_sitter::Range) -> &str {
    &text[range.start_byte..range.end_byte]
}
