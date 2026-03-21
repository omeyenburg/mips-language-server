use serde::de::value;
use tower_lsp_server::jsonrpc;
use tower_lsp_server::ls_types::request::{GotoDeclarationParams, GotoDeclarationResponse};
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use tree_sitter::{InputEdit, Point, Query, QueryCursor};

use streaming_iterator::StreamingIterator;

use crate::document;
use crate::lang::LanguageDefinitions;
use crate::lang::{Directive, Instruction, Registers};
use crate::server::{Backend, Documents};

impl Backend {
    // Goto definition: searches for word below cursor among all labels
    pub async fn do_goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> jsonrpc::Result<Option<GotoDefinitionResponse>> {
        // Unpack position and text_document
        let TextDocumentPositionParams {
            position,
            text_document,
        } = params.text_document_position_params;

        // Retrieve current content and tree
        let doc_arc = self
            .documents
            .get(&text_document.uri)
            .ok_or(jsonrpc::Error::invalid_request())?;

        let doc = doc_arc.read().await;
        let text = &doc.text;
        let tree = &doc.tree;

        // Determine node below cursor and fetch the label name
        let point = doc.position_to_point(&position);
        let cursor_label_name = tree
            .root_node()
            .descendant_for_point_range(
                point,
                Point {
                    row: point.row,
                    column: point.column + 1,
                },
            )
            .ok_or(jsonrpc::Error::invalid_request())?
            .utf8_text(text.as_bytes())
            .unwrap_or_default();

        // Generate query to find labels
        let query = Query::new(
            &tree_sitter_mips::LANGUAGE.into(),
            r#"
            (macro_label) @label
            (global_label) @label
            (local_label) @label
            (global_numeric_label) @label
            (local_numeric_label) @label
            "#,
        )
        .expect("Error compiling query");

        // Execute the query
        let mut query_cursor = QueryCursor::new();
        let mut matches = query_cursor.matches(&query, tree.root_node(), text.as_bytes());

        // Iterate over the matches
        while let Some(m) = matches.next() {
            if let Some(capture) = m.captures.first() {
                let node = capture.node;
                let label_text = &text[node.start_byte()..node.end_byte() - 1];

                if label_text == cursor_label_name {
                    return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                        uri: text_document.uri,
                        range: Range {
                            start: doc.point_to_position(&node.start_position()),
                            end: doc.point_to_position(&node.end_position()),
                        },
                    })));
                }
            }
        }

        // Return None; no definition found
        Ok(None)
    }
}
