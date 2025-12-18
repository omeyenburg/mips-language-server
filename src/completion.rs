use serde::de::value;
use tower_lsp_server::jsonrpc;
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use tree_sitter::{InputEdit, Query, QueryCursor};

use crate::language_definitions::{Directive, Instruction, Registers};
use crate::lsp::{Document, Documents};
use crate::server::Backend;

pub fn completion(
    backend: &Backend,
    params: CompletionParams,
) -> jsonrpc::Result<Option<CompletionResponse>> {
    let line = params.text_document_position.position.line;
    let character = params.text_document_position.position.character;

    // Retrieve current content and tree
    let document = backend
        .documents
        .get(&params.text_document_position.text_document.uri.into())
        .ok_or(jsonrpc::Error::invalid_request())?;
    let text = &document.text;

    // Split the text into lines and retrieve the specific line
    let line_content = text
        .lines()
        .nth(line as usize)
        .ok_or(jsonrpc::Error::invalid_request())?;

    // Find starting character of word that should be completed
    // Looks for '.' and '$' and returns ' ' if no match is found
    let mut starting_char = ' ';
    let mut starting_index = 0;
    for i in 0..character {
        starting_index = character - i - 1;
        if let Some(char) = line_content.chars().nth(starting_index as usize) {
            match char {
                '$' | '.' => starting_char = char,
                ' ' => {
                    // This is necessary to diffentiate from the loop
                    // ending, because the beginning of the line is reached
                    starting_index += 1;
                    break;
                }
                _ => starting_char = ' ',
            }
        }
    }

    // Generate completion items
    // Split up in: directive, register and instruction
    let items: Vec<CompletionItem> = match starting_char {
        '.' => backend
            .definitions
            .wait()
            .directives
            .iter()
            .map(|(mnemonic, directive)| {
                let expansion = String::from(".") + mnemonic.as_str();

                CompletionItem {
                    label: expansion.clone(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    detail: Some(format!("MIPS directive: .{}", mnemonic)),
                    label_details: Some(CompletionItemLabelDetails {
                        detail: None,
                        description: Some("directive".to_string()),
                    }),
                    documentation: Some(Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: directive.description.to_string(),
                    })),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range: Range {
                            start: Position {
                                line,
                                character: starting_index,
                            },
                            end: Position { line, character },
                        },
                        new_text: expansion,
                    })),
                    ..Default::default()
                }
            })
            .collect(),

        '$' => {
            let mut registers_common_float = backend.definitions.wait().registers.common.clone();

            let registers = if character - starting_index == 1 {
                // Return common & float
                for (key, value) in &backend.definitions.wait().registers.float {
                    registers_common_float.insert(key.to_string(), value.to_string());
                }
                &registers_common_float
            } else if let Some(char) = line_content.chars().nth(starting_index as usize + 1) {
                // Check context
                match char {
                    '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' => {
                        &backend.definitions.wait().registers.numeric
                    }
                    'f' => &backend.definitions.wait().registers.float,
                    _ => &backend.definitions.wait().registers.common,
                }
            } else {
                // Return common & float
                for (key, value) in &backend.definitions.wait().registers.float {
                    registers_common_float.insert(key.to_string(), value.to_string());
                }
                &registers_common_float
            };

            registers
                .keys()
                .map(|keyword| CompletionItem {
                    label: keyword.to_string(),
                    kind: Some(CompletionItemKind::VALUE),
                    detail: Some(format!("MIPS register: {}", keyword)),
                    label_details: Some(CompletionItemLabelDetails {
                        detail: None,
                        description: Some("register".to_string()),
                    }),
                    documentation: Some(Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: registers.get(keyword).unwrap().to_string(),
                    })),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range: Range {
                            start: Position {
                                line,
                                character: starting_index,
                            },
                            end: Position { line, character },
                        },
                        new_text: keyword.to_string(),
                    })),
                    ..Default::default()
                })
                .collect()
        }

        _ => {
            let good_named_func = |mnemonic: &String, instruction: &Instruction| CompletionItem {
                label: mnemonic.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some(format!("MIPS instruction: {}", mnemonic)),
                label_details: Some(CompletionItemLabelDetails {
                    detail: None,
                    description: Some("instruction".to_string()),
                }),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    // value: backend.short_instruction_docs(
                    //     keyword,
                    //     backend.instructions.get(keyword).unwrap(),
                    // ),
                    value: instruction.info.to_string(),
                })),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: Range {
                        start: Position {
                            line,
                            character: starting_index,
                        },
                        end: Position { line, character },
                    },
                    new_text: mnemonic.to_string(),
                })),
                ..Default::default()
            };

            // Completion is buggy when there is a dot.
            // This should filter instruction completions.
            if let Some(dot_position) = line_content[..character as usize].rfind(".") {
                let prefix = &line_content[starting_index as usize..character as usize];
                backend
                    .definitions
                    .wait()
                    .instructions
                    .iter()
                    .filter_map(|(mnemonic, instruction)| {
                        if mnemonic.starts_with(prefix) {
                            Some(good_named_func(
                                &(prefix.to_owned()
                                    + &mnemonic[character as usize - starting_index as usize..]),
                                instruction,
                            ))
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                backend
                    .definitions
                    .wait()
                    .instructions
                    .iter()
                    .map(|(mnemonic, instruction)| good_named_func(&mnemonic, instruction))
                    .collect()
            }
        }
    };

    // Return the completion items
    Ok(Some(CompletionResponse::List(
        tower_lsp_server::lsp_types::CompletionList {
            is_incomplete: false,
            items,
        },
    )))
}
