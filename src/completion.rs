use std::collections::HashMap;

use serde::de::value;
use tower_lsp_server::jsonrpc;
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use tree_sitter::{InputEdit, Query, QueryCursor};

use crate::lang::LanguageDefinitions;
use crate::lang::{Directive, Instruction, Registers};
use crate::server::{Backend, Document, Documents};

impl Backend {
    pub async fn get_completions(
        &self,
        params: CompletionParams,
    ) -> jsonrpc::Result<Option<CompletionResponse>> {
        let pos = params.text_document_position.position;

        // Retrieve current content and tree
        let arc_doc = self
            .documents
            .get(&params.text_document_position.text_document.uri.into())
            .ok_or(jsonrpc::Error::invalid_request())?;

        let doc = arc_doc.read().await;

        // Split the text into lines and retrieve the specific line
        let line_content = &doc
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
                    'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '$' | '%' | '\\' | '@' => {
                        starting_char = char
                    }
                    _ => break,
                }
            }

            starting_index -= 1;
        }

        let definitions: &LanguageDefinitions = self.definitions.wait();

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
    let start = starting_index as usize;
    let cursor = character as usize;

    if cursor < start || cursor > line_content.len() {
        return completion_response(Vec::new(), true);
    }

    let typed = &line_content[start..cursor];

    // Semantic filter up to the last dot, if any
    // Limits completion results, e.g. for "cmp."
    let dot_prefix = typed.rfind('.').map(|i| &typed[..=i]);

    let items = definitions
        .instructions
        .iter()
        .filter(|(mnemonic, _)| match dot_prefix {
            Some(p) => mnemonic.starts_with(p),
            None => true,
        })
        .map(|(mnemonic, instruction)| {
            completion_item(
                format!("MIPS instruction: {}", mnemonic),
                "instruction".to_string(),
                instruction.description.to_string(),
                CompletionItemKind::KEYWORD,
                mnemonic.to_string(),
                range,
            )
        })
        .collect();

    completion_response(items, true)
}
