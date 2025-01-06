use tower_lsp::lsp_types::Position;
use tracing::info;
use tree_sitter::*;

pub fn create_parser() -> Parser {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_mips::language()).unwrap();
    parser
}

pub fn byte_offset(text: &str, position: Position) -> usize {
    text.lines()
        .take(position.line as usize)
        .map(|line| line.len() + 1)
        .sum::<usize>()
        + position.character as usize
}

pub fn position_to_point(position: &Position) -> Point {
    Point {
        row: position.line as usize,
        column: position.character as usize,
    }
}

pub fn point_to_position(point: &Point) -> Position {
    Position {
        line: point.row as u32,
        character: point.column as u32,
    }
}

pub fn print_tree(node: tree_sitter::Node, indent: usize) {
    let mut cursor = node.walk();

    if node.kind() != "\n" {
        info!("|{}{:?}", "  ".repeat(indent), node.kind());
    }

    for child in node.children(&mut cursor) {
        print_tree(child, indent + 2);
    }
}
