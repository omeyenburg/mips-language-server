use serde::de::value;
use tower_lsp_server::jsonrpc;
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use tree_sitter::{InputEdit, Query, QueryCursor};

use crate::lang::LanguageDefinitions;
use crate::lang::{Directive, Instruction, Registers};
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

        log!("kind {}", kind);

        match kind {
            "opcode" => {
                if let Some(instruction) =
                    self.definitions.wait().instructions.get(cursor_node_text)
                {
                    return Ok(Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::String(
                            instruction.description.to_string(),
                        )),
                        range: None,
                    }));
                }
            }
            "macro_mnemonic" | "numeric_mnemonic" | "string_mnemonic" | "control_mnemonic" => {
                if let Some(directive) = self
                    .definitions
                    .wait()
                    .directives
                    .get(cursor_node_text.to_string()[1..].to_string().as_str())
                {
                    return Ok(Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::String(String::from(
                            directive.description.to_string(),
                        ))),
                        range: None,
                    }));
                }
            }
            "register" => {
                if let Some(register) = self
                    .definitions
                    .wait()
                    .registers
                    .numeric
                    .get(cursor_node_text)
                {
                    return Ok(Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::String(String::from(
                            register,
                        ))),
                        range: None,
                    }));
                }
                if let Some(register) = self
                    .definitions
                    .wait()
                    .registers
                    .common
                    .get(cursor_node_text)
                {
                    return Ok(Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::String(String::from(
                            register,
                        ))),
                        range: None,
                    }));
                }
                if let Some(register) = self
                    .definitions
                    .wait()
                    .registers
                    .float
                    .get(cursor_node_text)
                {
                    return Ok(Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::String(String::from(
                            register,
                        ))),
                        range: None,
                    }));
                }
            }
            _ => (),
        }

        Ok(None)
    }
}
