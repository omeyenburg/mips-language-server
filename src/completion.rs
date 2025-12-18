use std::collections::HashMap;

use serde::de::value;
use tower_lsp_server::jsonrpc;
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use tree_sitter::{InputEdit, Query, QueryCursor};

use crate::language_definitions::LanguageDefinitions;
use crate::language_definitions::{Directive, Instruction, Registers};
use crate::lsp::{Document, Documents};
use crate::server::Backend;

pub fn get_completions(
    backend: &Backend,
    params: CompletionParams,
) -> jsonrpc::Result<Option<CompletionResponse>> {
    // Retrieve current content and tree
    let document = backend
        .documents
        .get(&params.text_document_position.text_document.uri.into())
        .ok_or(jsonrpc::Error::invalid_request())?;

    let pos = params.text_document_position.position;

    // Split the text into lines and retrieve the specific line
    let line_content = &document
        .text
        .lines()
        .nth(pos.line as usize)
        .ok_or(jsonrpc::Error::invalid_request())?;

    // TODO: prevent completion in comments / make completions depend on place in syntax tree

    // Find starting character of word that should be completed
    // We start at the cursor position on the line and move right to the
    // beginning of the line until we hit some kind of separator:
    // ' ', '\t', ',' (separating symbols), ':', ';' after statement, '/' after block comment
    // The last seen char is saved. This might be '$', '.' or any other character.
    let mut starting_char = ' ';
    let mut starting_index = pos.character;

    while starting_index > 0 {
        if let Some(char) = line_content.chars().nth((starting_index - 1) as usize) {
            match char {
                'a'..='z' | '0'..='9' | '.' | '$' => starting_char = char,
                _ => break,
            }
        }

        starting_index -= 1;
    }

    let definitions: &LanguageDefinitions = backend.definitions.wait();

    let range = Range {
        start: Position {
            line: pos.line,
            character: starting_index,
        },
        end: pos,
    };

    // Generate completion items
    // Split up in: directive, register and instruction
    match starting_char {
        '.' => complete_directive(definitions, range),
        '$' => complete_register(
            definitions,
            range,
            line_content,
            pos.character,
            starting_index,
        ),
        _ => complete_instruction(
            definitions,
            range,
            line_content,
            pos.character,
            starting_index,
        ),
    }
}

fn completion_response(
    items: Vec<CompletionItem>,
    is_complete: bool,
) -> jsonrpc::Result<Option<CompletionResponse>> {
    Ok(Some(CompletionResponse::List(
        tower_lsp_server::lsp_types::CompletionList {
            items,
            is_incomplete: !is_complete,
        },
    )))
}

fn completion_item(
    detail: String,
    completion_type: String,
    documentation: String,
    kind: CompletionItemKind,
    expansion: String,
    range: Range,
) -> CompletionItem {
    CompletionItem {
        label: expansion.clone(),
        kind: Some(kind),
        detail: Some(detail),
        label_details: Some(CompletionItemLabelDetails {
            detail: None,
            description: Some(completion_type),
        }),
        documentation: Some(Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: documentation,
        })),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range,
            new_text: expansion,
        })),
        ..Default::default()
    }
}

fn complete_directive(
    definitions: &LanguageDefinitions,
    range: Range,
) -> jsonrpc::Result<Option<CompletionResponse>> {
    let items = definitions
        .directives
        .iter()
        .map(|(mnemonic, directive)| {
            completion_item(
                format!("MIPS directive: .{}", mnemonic),
                "directive".to_string(),
                directive.description.to_string(),
                CompletionItemKind::KEYWORD,
                String::from(".") + mnemonic.as_str(),
                range,
            )
        })
        .collect();

    completion_response(items, true)
}

fn complete_register(
    definitions: &LanguageDefinitions,
    range: Range,
    line_content: &str,
    character: u32,
    starting_index: u32,
) -> jsonrpc::Result<Option<CompletionResponse>> {
    let mut merged_registers;

    // Offer different completion lists based on context
    let registers = match line_content.chars().nth(starting_index as usize + 1) {
        Some('$') | None => {
            // Show float and common if char missing (unusual) or only $ was typed.
            // Numeric versions would be annoying here and are skipped.
            merged_registers = HashMap::new();
            merged_registers.extend(definitions.registers.float.clone());
            merged_registers.extend(definitions.registers.common.clone());
            &merged_registers
        }
        Some('0'..='9') => &definitions.registers.numeric,
        Some('f') => &definitions.registers.float,
        Some(_) => &definitions.registers.common,
    };

    let items = registers
        .iter()
        .map(|(keyword, description)| {
            completion_item(
                format!("MIPS register: {}", keyword),
                "register".to_string(),
                description.to_string(),
                CompletionItemKind::VALUE,
                keyword.to_string(),
                range,
            )
        })
        .collect();

    // Mark as incomplete, since we filtered by second character.
    completion_response(items, false)
}

fn complete_instruction(
    definitions: &LanguageDefinitions,
    range: Range,
    line_content: &str,
    character: u32,
    starting_index: u32,
) -> jsonrpc::Result<Option<CompletionResponse>> {
    // Editors/completion plugins often think that a dot is used for
    // separation but in mips a dot is part of a instruction mnemonic.
    // This filters instructions by the prefix up to the last typed dot.
    let prefix = &line_content[starting_index as usize..character as usize];

    let items = definitions
        .instructions
        .iter()
        .filter_map(|(mnemonic, instruction)| {
            if mnemonic.starts_with(prefix) {
                Some(completion_item(
                    format!("MIPS instruction: {}", mnemonic),
                    "instruction".to_string(),
                    instruction.description.to_string(),
                    CompletionItemKind::KEYWORD,
                    prefix.to_string() + &mnemonic[character as usize - starting_index as usize..],
                    range,
                ))
            } else {
                None
            }
        })
        .collect();

    completion_response(items, true)
}
