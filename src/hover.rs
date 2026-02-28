use serde::de::value;
use tower_lsp_server::jsonrpc;
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use tree_sitter::{InputEdit, Query, QueryCursor};

use crate::lang::{
    Directive, Directives, Instruction, Instructions, LanguageDefinitions, Registers,
};
use crate::server::{Backend, Document, Documents};

use crate::tree;

impl Backend {
    pub async fn hover(&self, params: HoverParams) -> jsonrpc::Result<Option<Hover>> {
        // Unpack position and text_document
        let TextDocumentPositionParams {
            position,
            text_document,
        } = params.text_document_position_params;

        // Retrieve current content and tree
        let arc_doc = self
            .documents
            .get(&text_document.uri)
            .ok_or(jsonrpc::Error::invalid_request())?;
        let doc = arc_doc.read().await;

        let text = &doc.text;
        let tree = doc.tree.clone();

        // Determine node below cursor and fetch the label name
        let point = tree::position_to_point(&position);
        let cursor_node = tree
            .root_node()
            .named_descendant_for_point_range(point, point)
            .ok_or(jsonrpc::Error::invalid_request())?;

        // Get label kind and text
        let kind = cursor_node.kind();
        let cursor_node_text = cursor_node.utf8_text(text.as_bytes()).unwrap_or_default();

        let definitions = self.definitions.read().await;

        let hover = match kind {
            "opcode" => hover_instruction(&definitions.instructions, cursor_node_text),
            "macro_mnemonic" | "numeric_mnemonic" | "string_mnemonic" | "control_mnemonic" => {
                hover_directive(&definitions.directives, cursor_node_text)
            }
            "register" => hover_register(&definitions.registers, cursor_node_text),
            _ => None,
        };

        Ok(hover)
    }
}

fn hover_instruction(instructions: &Instructions, text: &str) -> Option<Hover> {
    Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(
            instructions.get(text)?.description.to_string(),
        )),
        range: None,
    })
}

fn hover_directive(directives: &Directives, text: &str) -> Option<Hover> {
    let stripped_mnemonic = &text.to_string()[1..];

    directives.get(stripped_mnemonic).map(|directive| Hover {
        contents: HoverContents::Scalar(MarkedString::String(directive.description.to_string())),
        range: None,
    })
}

fn hover_register(registers: &Registers, text: &str) -> Option<Hover> {
    if let Some(register) = registers.numeric.get(text) {
        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(String::from(register))),
            range: None,
        })
    } else if let Some(register) = registers.common.get(text) {
        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(String::from(register))),
            range: None,
        })
    } else {
        registers.float.get(text).map(|register| Hover {
            contents: HoverContents::Scalar(MarkedString::String(String::from(register))),
            range: None,
        })
    }
}
