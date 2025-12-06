use std::collections::HashMap;

use streaming_iterator::StreamingIterator;
use tower_lsp::lsp_types::*;
use tree_sitter::{Node, Query, QueryCursor, TreeCursor};

use crate::json::Directives;
use crate::json::Instructions;
use crate::lsp::Document;
use crate::tree;

fn add_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    node: &Node,
    message: &str,
    severity: DiagnosticSeverity,
) {
    let start = tree::point_to_position(&node.start_position());
    let end = tree::point_to_position(&node.end_position());
    diagnostics.push(Diagnostic {
        range: Range { start, end },
        severity: Some(severity),
        code: None,
        code_description: None,
        source: Some("mips-language-server".to_string()),
        message: message.to_string(),
        related_information: None,
        tags: None,
        data: None,
    });
}

#[derive(Debug)]
struct Arg {
    name: String,
    range: Range,
}

#[derive(Debug)]
struct ParsedInstruction {
    mnemonic: String,
    range: Range,
    args: Vec<Arg>,
}

#[derive(Debug)]
struct ParsedDirective {
    kind: String,
    range: Range,
    args: Vec<Arg>,
}

#[derive(Debug)]
struct ParsedLabel {
    name: String,
    range: Range,
}

#[derive(Debug)]
struct Section {
    range: Range,
    instructions: Vec<ParsedInstruction>,
    directives: Vec<ParsedDirective>,
    labels: Vec<ParsedLabel>,
}

#[derive(Debug)]
struct Sections {
    unknown: Section,
    data: Section,
    kdata: Section,
    sdata: Section,
    rdata: Section,
    ktext: Section,
    text: Section,
    bss: Section,
    sbss: Section,
}

#[derive(Debug)]
struct MacroDefinition {
    name: String,
    args: Vec<Arg>,
    header_range: Range,
    body_ranges: Vec<Range>,
}

pub fn parse(diagnostics: &mut Vec<Diagnostic>, document: &Document) {
    let Document { tree, text } = document;

    let directive_nodes = parse_raw_directives(tree, text);

    let mut raw_sections: HashMap<usize, &str> = HashMap::new();
    raw_sections.insert(0, "unknown");

    let mut sections: Sections;

    let mut macros: Vec<MacroDefinition> = Vec::new();
    let mut macro_stack: Vec<usize> = Vec::new();
    let mut last_macro_start = None;

    // TODO: rename directive attributes and move parsing elsewhere
    directive_nodes.iter().for_each(|node| {
        if let Some(meta) = node.child(0) {
            let attributes: Vec<Node> = if let Some(attributes) = node.child(1) {
                let mut cursor = attributes.walk();
                attributes.children(&mut cursor).collect()
            } else {
                Vec::new()
            };

            let meta_text = &text[meta.byte_range()];
            let start_byte = node.start_byte();
            let start = tree::point_to_position(&node.start_position());
            let end = tree::point_to_position(&node.end_position());

            match meta_text {
                ".data" | ".kdata" | ".sdata" | ".rdata" | ".text" | ".ktext" | ".bss"
                | ".sbss" => {
                    raw_sections.insert(start_byte, meta_text);
                }
                ".macro" => {
                    if let Some(last_index) = macro_stack.last() {
                        if let Some(last_start) = last_macro_start {
                            macros[*last_index].body_ranges.push(Range {
                                start: last_start,
                                end: start,
                            });
                        }
                    }

                    let name = if let Some(name_node) = attributes.first() {
                        &text[name_node.byte_range()]
                    } else {
                        ""
                    };

                    let macro_def = MacroDefinition {
                        name: name.to_string(),
                        args: attributes.get(1..).unwrap_or(&[])
                            .iter()
                            .map(|x| Arg {
                                name: text[x.byte_range()].to_string(),
                                range: Range {
                                    start: tree::point_to_position(&x.start_position()),
                                    end: tree::point_to_position(&x.end_position()),
                                },
                            })
                            .collect(),
                        header_range: Range { start, end },
                        body_ranges: Vec::new(),
                    };
                    macros.push(macro_def);
                    macro_stack.push(macros.len() - 1);
                    last_macro_start = Some(start);
                }
                ".endm" | ".macro_end" => {
                    if let Some(last_index) = macro_stack.last() {
                        if let Some(last_start) = last_macro_start {
                            macros[*last_index].body_ranges.push(Range {
                                start: last_start,
                                end,
                            });
                        }
                    }

                    macro_stack.pop();
                    last_macro_start = Some(end);
                }
                _ => {
                    log!("unhandled directive: {}", meta_text);
                }
            }
        };
    });

    log!("{:?}", macros);
}

// First iteration of parsing: find all section uning directives.
// This can later be used as a lookup table to determine to which section a statement belongs.
// The sections also contain macro directives!
// This function ALSO parse other directives!
fn parse_raw_directives<'a>(tree: &'a tree_sitter::Tree, text: &'a String) -> Vec<Node<'a>> {
    // let mut raw_sections: HashMap<usize, &str> = HashMap::new();
    let mut directive_nodes = Vec::new();

    // Query directives in syntax tree
    let t: tree_sitter::Language = tree_sitter_mips::LANGUAGE.into();
    let query = Query::new(&t, r#"(directive) @directive"#)
        .expect("Error compiling query");
    let mut query_cursor = QueryCursor::new();
    let mut matches = query_cursor.matches(&query, tree.root_node(), text.as_bytes());

    while let Some(m) = matches.next() {
        if let Some(capture) = m.captures.first() {
            let node = capture.node;
            // let meta_text = &text[node.byte_range()];
            // let start_byte = node.start_byte();

            // match meta_text {
            //     ".data" | ".kdata" | ".sdata" | ".rdata" | ".text" | ".ktext" | ".bss"
            //     | ".sbss" | ".macro" | ".endm" | ".end_macro" => {
            //         raw_sections.insert(start_byte, meta_text);
            //     }
            //     _ => {
            directive_nodes.push(node);
            //     }
            // }
        }
    }

    directive_nodes
}

fn parse_diretives() {}

// pub fn parse_directives(
//     diagnostics: &mut Vec<Diagnostic>,
//     document: &Document,
//     directives: &Directives,
// ) {
//     let Document { tree, text } = document;

//     // Generate query to find directives
//     let query = Query::new(&tree_sitter_mips::language(), r#"(meta) @meta"#)
//         .expect("Error compiling query");

//     // Execute the query
//     let mut query_cursor = QueryCursor::new();
//     let mut matches = query_cursor.matches(&query, tree.root_node(), text.as_bytes());

//     // Create hashset to collect data nodes
//     let mut data_nodes = std::collections::HashSet::new();

//     // Starting points of each section
//     let mut sections = std::collections::HashMap::new();

//     // Iterate over the matches
//     while let Some(m) = matches.next() {
//         if let Some(capture) = m.captures.first() {
//             let node = capture.node;
//             let meta_text = &text[node.start_byte()..node.end_byte()];
//             let start = tree::point_to_position(&node.start_position());
//             let end = tree::point_to_position(&node.end_position());

//             if !directives.contains_key(meta_text) {
//                 add_diagnostic(
//                     diagnostics,
//                     &node,
//                     "Unknown directive",
//                     DiagnosticSeverity::ERROR,
//                 );
//                 continue;
//             }

//             match meta_text {
//                 ".data" | ".kdata" | ".sdata" | ".rdata" | ".text" | ".ktext" | ".bss"
//                 | ".sbss" => {
//                     //if sections.contains_key(meta_text) {
//                     //    add_diagnostic(
//                     //        diagnostics,
//                     //        &node,
//                     //        format!("Multiple {} sections", meta_text).as_str(),
//                     //        DiagnosticSeverity::ERROR,
//                     //    );
//                     //    continue;
//                     //}
//                     sections.insert(start.line, meta_text);
//                 }
//                 ".word" | ".ascii" | ".asciiz" | ".byte" | ".align" | ".half" | ".space"
//                 | ".double" | ".float" => {
//                     data_nodes.insert(node);
//                 }
//                 _ => {}
//             }
//         }
//     }

//     // Parse sections
//     let mut text_section: Vec<Range> = vec![]; // text, ktext
//     let mut data_section: Vec<Range> = vec![]; // data, kdata, sdata,
//     let mut rdata_section: Vec<Range> = vec![]; // rdata
//     let mut bss_section: Vec<Range> = vec![]; // bss, sbss

//     let mut current_name = ".text";
//     let mut current_line = 0;

//     for (line, name) in &sections {
//         let section = match current_name {
//             ".text" | ".ktext" => &mut text_section,
//             ".data" | ".kdata" | ".sdata" => &mut data_section,
//             ".rdata" => &mut rdata_section,
//             _ => &mut bss_section,
//         };
//         section.push(Range {
//             start: Position {
//                 character: 0,
//                 line: current_line,
//             },
//             end: Position {
//                 character: 0,
//                 line: *line,
//             },
//         });
//         current_name = name;
//         current_line = *line;

//         log!("{}", current_line);
//         log!("{}", current_name);
//     }
// }

// pub fn parse_instructions(
//     diagnostics: &mut Vec<Diagnostic>,
//     document: &Document,
//     instructions: &Instructions,
// ) {
//     let Document { tree, text } = document;

//     // Generate query to find instructions
//     let query = Query::new(
//         &tree_sitter_mips::language(),
//         r#"(instruction) @instruction"#,
//     )
//     .expect("Error compiling query");

//     // Execute the query
//     let mut query_cursor = QueryCursor::new();
//     let mut matches = query_cursor.matches(&query, tree.root_node(), text.as_bytes());

//     // Iterate over the matches
//     while let Some(m) = matches.next() {
//         if let Some(capture) = m.captures.first() {
//             let node = capture.node;
//             let mut cursor = node.walk();
//             let mut instruction_name;

//             for child in node.children(&mut cursor) {
//                 if child.kind() == "opcode" {
//                     instruction_name = &text[child.start_byte()..child.end_byte()];
//                     log!("inst:{}!", instruction_name);
//                     if !instructions.contains_key(instruction_name) {
//                         let start = tree::point_to_position(&node.start_position());
//                         let end = tree::point_to_position(&node.end_position());
//                         add_diagnostic(
//                             diagnostics,
//                             &node,
//                             "Unknown instruction",
//                             DiagnosticSeverity::ERROR,
//                         );
//                     }
//                 }
//             }
//         }
//     }
// }

// pub fn parse_labels(diagnostics: &mut Vec<Diagnostic>, document: &Document) {
//     let Document { tree, text } = document;

//     // Generate query to find labels
//     let query = Query::new(&tree_sitter_mips::language(), r#"(label) @label"#)
//         .expect("Error compiling query");

//     // Execute the query
//     let mut query_cursor = QueryCursor::new();
//     let mut matches = query_cursor.matches(&query, tree.root_node(), text.as_bytes());

//     // Create hashset to collect label names
//     let mut label_texts = std::collections::HashSet::new();

//     // Iterate over the matches
//     while let Some(m) = matches.next() {
//         if let Some(capture) = m.captures.first() {
//             let node = capture.node;
//             let label_text = &text[node.start_byte()..node.end_byte()];

//             if label_texts.contains(label_text) {
//                 log!("Duplicate label! {}", label_text);
//                 let start = tree::point_to_position(&node.start_position());
//                 let end = tree::point_to_position(&node.end_position());
//                 add_diagnostic(
//                     diagnostics,
//                     &node,
//                     "Duplicate label",
//                     DiagnosticSeverity::ERROR,
//                 );
//             } else {
//                 label_texts.insert(label_text);
//             }
//         }
//     }
// }

// // Parse an argument and return it's type
// // Examples:
// //  [register]: $t0, $s1, $a0, $v0
// //  [float register]: $f0, $f1, $f31, $f12
// //  [invalid register]: $abc
// //  [immediate]: 24, 2134, 0x3fa, 0274124, 94967296
// //  [address]: x, y, x($t0), 4($t0), 0xa8($s1)
// //  [jump label]: main, loop
// fn parse_argument(argument: &str) -> Option<&str> {
//     if argument.is_empty() {
//         return None;
//     }

//     let registers = [
//         "$0", "$1", "$2", "$3", "$4", "$5", "$6", "$7", "$8", "$9", "$10", "$11", "$12", "$13",
//         "$14", "$15", "$16", "$17", "$18", "$19", "$20", "$21", "$22", "$23", "$24", "$25", "$26",
//         "$27", "$28", "$29", "$30", "$31", "$zero", "$at", "$v0", "$v1", "$a0", "$a1", "$a2",
//         "$a3", "$t0", "$t1", "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t8", "$t9", "$s0", "$s1",
//         "$s2", "$s3", "$s4", "$s5", "$s6", "$s7", "$k0", "$k1", "$gp", "$sp", "$fp", "$ra",
//     ];
//     let float_registers = [
//         "$f0", "$f1", "$f2", "$f3", "$f4", "$f5", "$f6", "$f7", "$f8", "$f9", "$f10", "$f11",
//         "$f12", "$f13", "$f14", "$f15", "$f16", "$f17", "$f18", "$f19", "$f20", "$f21", "$f22",
//         "$f23", "$f24", "$f25", "$f26", "$f27", "$f28", "$f29", "$f30", "$f31",
//     ];

//     // Check if the argument is a register
//     if argument.starts_with("$") {
//         if registers.contains(&argument) {
//             return Some("register");
//         }
//         if float_registers.contains(&argument) {
//             return Some("float register");
//         }
//         return Some("invalid register");
//     }

//     // Assume argument is decimal immediate or address
//     let mut rmap: fn(char) -> bool;
//     rmap = |x| x.is_ascii_digit();
//     let mut rtype = "immediate";

//     // Ignore sign prefix
//     let mut offset = if argument.starts_with("-") { 1 } else { 0 };

//     let bytes = argument.as_bytes();
//     if bytes[0].is_ascii_alphabetic() {
//         // Argument is label or address (if valid)
//         rmap = |byte| byte.is_ascii_alphanumeric();
//         rtype = "label";
//     } else if argument.starts_with("0x") {
//         // Argument is immediate or address (if valid)
//         rmap = |byte| byte.is_ascii_hexdigit();
//         rtype = "immediate";
//         offset += 2;
//     }

//     let mut bracket = false;
//     let mut sub_start = 0;

//     for (i, char) in argument.chars().enumerate() {
//         if i < offset || rmap(char) {
//             continue;
//         }

//         if char == '(' {
//             if bracket || sub_start != 0 {
//                 // When a bracket was already found
//                 return Some("invalid address");
//             }
//             bracket = true;
//             sub_start = i + 1;
//         }

//         if char == ')' {
//             if i + 1 != argument.len() || !bracket {
//                 // When no opening bracket was found or symbols follow closing bracket
//                 return Some("invalid address");
//             }
//             bracket = false;
//         }
//     }

//     // If brackets were found, an address is expected
//     if sub_start > 0 {
//         if bracket {
//             // missing closing bracket
//             return Some("address missing bracket");
//         }

//         let sub_string = &argument[sub_start..argument.len() - 1];
//         if registers.contains(&sub_string) {
//             return Some("address");
//         }

//         // Substring in brackets is not a register
//         return Some("invalid address");
//     }

//     // Valid argument, return type
//     Some(rtype)
// }

// fn examples(string: &str) -> String {
//     string
//         .replacen("[register]", "$t1", 1)
//         .replacen("[register]", "$t2", 1)
//         .replacen("[register]", "$t3", 1)
//         .replacen("[float register]", "$f0", 1)
//         .replacen("[float register]", "$f2", 1)
//         .replacen("[float register]", "$f4", 1)
//         .replacen("[coprocessor register]", "$8", 1)
//         .replacen("[address]", "4($t1)", 1)
//         .replacen("[jump label]", "target", 1)
// }
