use std::ops::Index;

use smol_str::ToSmolStr;
use tree_sitter::{Node, Parser, Point, Tree, TreeCursor};

use crate::ast::*;

impl Ast {
    pub fn from_ts_tree(text: &str, tree: &Tree) -> Self {
        let root = tree.root_node();
        parse_file(text, &root)
    }
}

fn parse_file<'a>(text: &str, node: &Node<'a>) -> Ast {
    let range = node.range();

    if node.kind() != "program" {
        return Ast {
            range,
            items: Vec::new(),
        };
    }

    let items = node
        .named_children(&mut node.walk())
        .filter_map(|child| match child.kind() {
            "instruction" => Some(parse_instruction(text, &child, false)),
            "directive" => Some(parse_directive(text, &child)),
            "call_instruction" => Some(parse_instruction(text, &child, true)),
            "macro_label" => Some(parse_label(text, &child, LabelKind::Macro)),
            "label" => Some(parse_label(text, &child, LabelKind::Normal)),
            "numeric_label" => Some(parse_label(text, &child, LabelKind::Numeric)),
            "comment" => None,
            _ => Some(SyntaxNode::Error(SyntaxErrorNode {
                range: child.range(),
            })),
        })
        .collect();

    Ast { range, items }
}

fn parse_directive<'a>(text: &str, node: &Node<'a>) -> SyntaxNode {
    let Some(mnemonic) = node.child_by_field_name("mnemonic") else {
        return SyntaxNode::Error(SyntaxErrorNode {
            range: node.range(),
        });
    };

    if mnemonic.kind() == "macro_mnemonic" {
        let name = node.child_by_field_name("name").map(|node| Identifier {
            range: node.range(),
        });
        let parameters = parse_macro_parameters(text, node);
        SyntaxNode::MacroDefinition(MacroDefinitionNode {
            name,
            parameters,
            range: node.range(),
        })
    } else {
        let mnemonic = Identifier {
            range: mnemonic.range(),
        };
        let operands = parse_operands(text, node.child_by_field_name("operands"));
        SyntaxNode::Directive(DirectiveNode {
            mnemonic,
            operands,
            range: node.range(),
        })
    }
}

fn parse_macro_parameters<'a>(text: &str, node: &Node<'a>) -> Vec<MacroParameterNode> {
    node.child_by_field_name("parameters")
        .map(|node: Node| {
            node.named_children(&mut node.walk())
                .filter_map(|node| {
                    let name_node = node.child_by_field_name("name")?;

                    let qualifier_node = node.child_by_field_name("qualifier");
                    let value_node = node.child_by_field_name("value");

                    Some(MacroParameterNode {
                        name: Identifier {
                            range: name_node.range(),
                        },
                        qualifier: qualifier_node.map(|node| Identifier {
                            range: node.range(),
                        }),
                        value: value_node.map(|node| parse_expression(text, &node)),
                        range: node.range(),
                    })
                })
                .collect()
        })
        .unwrap_or_else(|| Vec::with_capacity(0))
}

fn parse_label<'a>(text: &str, node: &Node<'a>, kind: LabelKind) -> SyntaxNode {
    let range = node.range();
    let mut name_range = range;

    if text.as_bytes()[range.end_byte - 1] != b':' {
        log!("missing colon");
        return SyntaxNode::Error(SyntaxErrorNode { range });
    }

    name_range.end_byte -= 1;

    SyntaxNode::Label(LabelNode {
        kind,
        name: Identifier { range: name_range },
        range,
    })
}

fn parse_instruction<'a>(text: &str, node: &Node<'a>, is_call: bool) -> SyntaxNode {
    let mnemonic_node = node.child_by_field_name("mnemonic");
    let operands_node = node.child_by_field_name("operands");
    let range = node.range();

    let Some(mnemonic_node) = mnemonic_node else {
        return SyntaxNode::Error(SyntaxErrorNode { range });
    };

    let mnemonic = Identifier {
        range: mnemonic_node.range(),
    };
    let operands = parse_operands(text, operands_node);

    if is_call {
        SyntaxNode::MacroInvocation(MacroInvocationNode {
            mnemonic,
            operands,
            range,
        })
    } else {
        SyntaxNode::Instruction(InstructionNode {
            mnemonic,
            operands,
            range,
        })
    }
}

fn parse_operands(text: &str, node: Option<Node>) -> Vec<OperandListItem> {
    let Some(operands_node) = node else {
        return Vec::with_capacity(0);
    };

    operands_node
        .children(&mut operands_node.walk())
        .filter_map(|node| match node.kind() {
            "," => Some(OperandListItem::Comma(node.range())),
            "_expression" => Some(OperandListItem::Operand(parse_expression(
                text,
                &node.first_named_child_for_byte(0)?,
            ))),
            "ERROR" => Some(OperandListItem::MissingOperand(node.range())),
            _ => None,
        })
        .collect()
}

fn parse_expression<'a>(text: &str, node: &Node<'a>) -> ValueNode {
    let range = node.range();

    match node.kind() {
        "macro_variable" => ValueNode::MacroVariable { range },
        "register" => ValueNode::Register { range },
        "numeric_label_reference" => ValueNode::NumericLabelReference { range },
        "symbol" => ValueNode::Symbol { range },
        "elf_type_tag" => ValueNode::ElfTypeTag { range },
        "option_flag" => ValueNode::OptionFlag { range },
        "char" => parse_char(text, node),
        "string_concatenation" => parse_string_concat(text, node),
        "string" => parse_string(text, node),
        "octal" => parse_octal(text, node),
        "binary" => parse_binary(text, node),
        "decimal" => parse_decimal(text, node),
        "hexadecimal" => parse_hexadecimal(text, node),
        "float" => parse_float(text, node),
        "unary_expression" => parse_unary_expr(text, node),
        "parenthesized_expression" => parse_parenthesized_expr(text, node),
        "binary_expression" => parse_binary_expr(text, node),
        _ => ValueNode::MalformedValue { range },
    }
}

fn parse_string<'a>(text: &str, node: &Node<'a>) -> ValueNode {
    let range = node.range();

    let macro_variables = node
        .named_children(&mut node.walk())
        .map(|node| {
            if node.kind() == "string_macro_variable" {
                ValueNode::MacroVariable { range }
            } else {
                ValueNode::MalformedValue { range }
            }
        })
        .collect();

    ValueNode::String {
        range,
        macro_variables,
    }
}

fn parse_char(text: &str, node: &Node) -> ValueNode {
    let range = node.range();
    let s = &text[node.start_byte()..node.end_byte()];

    // Strip single quotes around char value
    if s.len() < 2 {
        return ValueNode::MalformedValue { range };
    }
    let inner = &s[1..s.len() - 1];

    let value = if !inner.starts_with('\\') {
        let Some(value) = inner.chars().next() else {
            return ValueNode::MalformedValue { range };
        };
        value
    } else {
        let esc = &inner[1..];
        match esc {
            "n" => '\n',
            "r" => '\r',
            "t" => '\t',
            "\\" => '\\',
            "'" => '\'',
            "0" => '\0',
            hex if hex.starts_with('x') && hex.len() == 3 => {
                if let Ok(byte) = u8::from_str_radix(&hex[1..], 16) {
                    byte as char
                } else {
                    return ValueNode::MalformedValue { range };
                }
            }
            _ => return ValueNode::MalformedValue { range },
        }
    };

    ValueNode::Char { value, range }
}

fn parse_string_concat<'a>(text: &str, node: &Node<'a>) -> ValueNode {
    let range = node.range();

    let macro_variables = node
        .named_children(&mut node.walk())
        .filter_map(|node| match node.kind() {
            "macro_variable" => Some(ValueNode::MacroVariable { range }),
            "string" => None,
            _ => Some(ValueNode::MalformedValue { range }),
        })
        .collect();

    ValueNode::String {
        range,
        macro_variables,
    }
}

/// Slice the node text safely
fn node_text<'a>(text: &'a str, node: &Node<'a>) -> &'a str {
    &text[node.start_byte()..node.end_byte()]
}

fn parse_non_dec_value(text: &str, node: &Node, radix: u32) -> Option<i64> {
    let s = node_text(text, node);

    // Remove optional "0o" prefix
    let s = s
        .strip_prefix("-0o")
        .or_else(|| s.strip_prefix("0o"))
        .or_else(|| s.strip_prefix("-0x"))
        .or_else(|| s.strip_prefix("0x"))
        .or_else(|| s.strip_prefix("-0b"))
        .or_else(|| s.strip_prefix("0b"))
        .unwrap_or(s);

    // Handle optional negative sign manually
    let negative = s.starts_with('-');
    let digits = if negative { &s[1..] } else { s };

    let range = node.range();

    i64::from_str_radix(digits, radix)
        .ok()
        .map(|v| if negative { -v } else { v })
}

/// Parse octal: "-0o755" or "0755"
fn parse_octal(text: &str, node: &Node) -> ValueNode {
    let range = node.range();

    if let Some(value) = parse_non_dec_value(text, node, 8) {
        ValueNode::Octal { value, range }
    } else {
        ValueNode::MalformedValue { range }
    }
}

/// Parse binary: "-0b1010"
fn parse_binary(text: &str, node: &Node) -> ValueNode {
    let range = node.range();

    if let Some(value) = parse_non_dec_value(text, node, 2) {
        ValueNode::Binary { value, range }
    } else {
        ValueNode::MalformedValue { range }
    }
}

/// Parse decimal: "-42"
fn parse_decimal(text: &str, node: &Node) -> ValueNode {
    let s = node_text(text, node);
    let range = node.range();

    if let Ok(value) = s.parse::<i64>() {
        ValueNode::Decimal { value, range }
    } else {
        ValueNode::MalformedValue { range }
    }
}

/// Parse hexadecimal: "-0xFF"
fn parse_hexadecimal(text: &str, node: &Node) -> ValueNode {
    let range = node.range();

    if let Some(value) = parse_non_dec_value(text, node, 16) {
        ValueNode::Binary { value, range }
    } else {
        ValueNode::MalformedValue { range }
    }
}

/// Parse float: "-3.14", "2e10", "2f"
fn parse_float(text: &str, node: &Node) -> ValueNode {
    let s = node_text(text, node).trim_end_matches(['f', 'd']);
    let range = node.range();

    if let Ok(value) = s.parse::<f64>() {
        ValueNode::Float { value, range }
    } else {
        ValueNode::MalformedValue { range }
    }
}

fn parse_binary_expr(text: &str, node: &Node) -> ValueNode {
    let range = node.range();

    let Some(left_node) = node.child_by_field_name("left") else {
        return ValueNode::MalformedValue { range };
    };
    let Some(operator_node) = node.child_by_field_name("operator") else {
        return ValueNode::MalformedValue { range };
    };
    let Some(right_node) = node.child_by_field_name("right") else {
        return ValueNode::MalformedValue { range };
    };

    let operator_kind = match operator_node.kind() {
        "additive_operator" => OperatorKind::Additive,
        "assignment_operator" => OperatorKind::Assignment,
        "bitwise_and_operator" => OperatorKind::BitwiseAnd,
        "bitwise_or_operator" => OperatorKind::BitwiseOr,
        "bitwise_xor_operator" => OperatorKind::BitwiseXor,
        "equality_operator" => OperatorKind::Equality,
        "logical_and_operator" => OperatorKind::LogicalAnd,
        "logical_or_operator" => OperatorKind::LogicalOr,
        "multiplicative_operator" => OperatorKind::Multiplicative,
        "relational_operator" => OperatorKind::Relational,
        "shift_operator" => OperatorKind::Shift,
        _ => return ValueNode::MalformedValue { range },
    };

    ValueNode::BinaryExpression {
        left: Box::new(parse_expression(text, &left_node)),
        right: Box::new(parse_expression(text, &right_node)),
        operator: OperatorNode {
            kind: operator_kind,
            range: operator_node.range(),
        },
        range,
    }
}

fn parse_unary_expr(text: &str, node: &Node) -> ValueNode {
    let range = node.range();

    let Some(operator_node) = node.child_by_field_name("operator") else {
        return ValueNode::MalformedValue { range };
    };
    let Some(argument_node) = node.child_by_field_name("argument") else {
        return ValueNode::MalformedValue { range };
    };

    let operator_kind = match operator_node.kind() {
        "unary_minus_operator" => UnaryOperatorKind::Negation,
        "bitwise_not_operator" => UnaryOperatorKind::BitwiseNegation,
        "logical_not_operator" => UnaryOperatorKind::LogicalNegation,
        _ => return ValueNode::MalformedValue { range },
    };

    ValueNode::UnaryExpression {
        body: Box::new(parse_expression(text, &argument_node)),
        operator: UnaryOperatorNode {
            kind: operator_kind,
            range: operator_node.range(),
        },
        range,
    }
}

fn parse_parenthesized_expr(text: &str, node: &Node) -> ValueNode {
    let range = node.range();

    let head_node = node.child_by_field_name("head");
    let body_node = node.child_by_field_name("arguments");

    ValueNode::ParenthesizedExpression {
        head: head_node.map(|node| Box::new(parse_expression(text, &node))),
        body: parse_operands(text, Some(*node)),
        range,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_float() {
        // assert_eq!(parse_float(1, 2), 3);
    }
}
