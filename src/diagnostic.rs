use std::collections::HashMap;

use serde::de::value;
use smol_str::SmolStr;
use tower_lsp_server::jsonrpc;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use tree_sitter::{InputEdit, Query, QueryCursor};

use crate::ast::SyntaxNode;
use crate::document::Document;
use crate::lang::LanguageDefinitions;
use crate::lang::{Directive, Instruction, Registers};
use crate::server::{Backend, Documents};

use crate::document;
use crate::semantic;

impl Document {
    pub async fn analyze_document(&self) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for err in &self.semantic_model.syntax_errors {
            let diagnostic = match err {
                semantic::Error::InvalidSyntax(r) => get_syntax_error_diagnostic(self, r),
                semantic::Error::MalformedOperand(r) => get_malformed_operand_diagnostic(self, r),
                semantic::Error::MissingMacroName(r) => get_missing_macro_name_diagnostic(self, r),
                semantic::Error::DuplicateLabel { range: r, name } => {
                    get_duplicate_label_diagnostic(self, r, name)
                }
                semantic::Error::DuplicateMacroName { range: r, name } => {
                    get_duplicate_macro_name_diagnostic(self, r, name)
                }
            };

            if let Some(diagnostic) = diagnostic {
                diags.push(diagnostic);
            }
        }

        diags
    }
}

fn get_syntax_error_diagnostic(doc: &Document, range: &tree_sitter::Range) -> Option<Diagnostic> {
    Some(create_diagnostic(
        range,
        "E001",
        "error: invalid syntax",
        DiagnosticSeverity::ERROR,
        None,
    ))
}

fn get_malformed_operand_diagnostic(
    doc: &Document,
    range: &tree_sitter::Range,
) -> Option<Diagnostic> {
    Some(create_diagnostic(
        range,
        "E002",
        "error: malformed operand",
        DiagnosticSeverity::ERROR,
        None,
    ))
}

fn get_missing_macro_name_diagnostic(
    doc: &Document,
    range: &tree_sitter::Range,
) -> Option<Diagnostic> {
    Some(create_diagnostic(
        range,
        "E003",
        "error: missing macro name",
        DiagnosticSeverity::ERROR,
        None,
    ))
}

fn get_duplicate_label_diagnostic(
    doc: &Document,
    range: &tree_sitter::Range,
    name: &str,
) -> Option<Diagnostic> {
    let Some(label) = doc.semantic_model.labels.get(name) else {
        debug_assert!(false, "failed to look up previous label definition");
        return None;
    };
    let Some(node) = doc.ast.items.get(label.statement_index) else {
        debug_assert!(false, "model & ast inconsistent: index out of bounds");
        return None;
    };
    let SyntaxNode::Label(label_node) = node else {
        debug_assert!(false, "model & ast inconsistent: wrong type at index");
        return None;
    };

    let location = Location {
        uri: doc.uri.clone(),
        range: document::utils::ts_range_to_ls_range(&label_node.range),
    };
    Some(create_diagnostic(
        range,
        "E004",
        "error: label already defined",
        DiagnosticSeverity::ERROR,
        create_single_related_information(
            &doc.uri,
            &label_node.range,
            "label previously defined here",
        ),
    ))
}

fn get_duplicate_macro_name_diagnostic(
    doc: &Document,
    range: &tree_sitter::Range,
    name: &str,
) -> Option<Diagnostic> {
    let Some(macro_def) = doc.semantic_model.macros.get(name) else {
        debug_assert!(false, "failed to look up previous macro definition");
        return None;
    };
    let Some(node) = doc.ast.items.get(macro_def.statement_index) else {
        debug_assert!(false, "model & ast inconsistent: index out of bounds");
        return None;
    };
    let SyntaxNode::MacroDefinition(macro_def_node) = node else {
        debug_assert!(false, "model & ast inconsistent: wrong type at index");
        return None;
    };

    Some(create_diagnostic(
        range,
        "E005",
        "error: macro name already defined",
        DiagnosticSeverity::ERROR,
        create_single_related_information(
            &doc.uri,
            &macro_def_node.range,
            "macro name previously defined here",
        ),
    ))
}

fn create_diagnostic(
    range: &tree_sitter::Range,
    code: &str,
    message: &str,
    severity: DiagnosticSeverity,
    related_information: Option<Vec<DiagnosticRelatedInformation>>,
) -> Diagnostic {
    Diagnostic {
        range: document::utils::ts_range_to_ls_range(range),
        severity: Some(severity),
        code: Some(NumberOrString::String(code.to_string())),
        code_description: None,
        source: Some("mipsls".to_string()),
        message: message.to_string(),
        related_information,
        tags: None,
        data: None,
    }
}

fn create_single_related_information(
    uri: &Uri,
    range: &tree_sitter::Range,
    message: &str,
) -> Option<Vec<DiagnosticRelatedInformation>> {
    Some(vec![DiagnosticRelatedInformation {
        location: Location {
            uri: uri.clone(),
            range: document::utils::ts_range_to_ls_range(range),
        },
        message: message.to_string(),
    }])
}
