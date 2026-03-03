use std::collections::HashMap;

use smol_str::{SmolStr, ToSmolStr};
use tree_sitter::Range;

use crate::ast::*;
use crate::document::utils::get_text_in_ts_range;

pub struct SemanticModel {
    pub syntax_errors: Vec<Error>,
    pub labels: HashMap<SmolStr, Label>,
    pub macros: HashMap<SmolStr, MacroDefinition>,
    pub directives: Vec<Directive>,
    pub instructions: Vec<Instruction>,
}

pub enum Error {
    MalformedOperand(Range),
    InvalidSyntax(Range),
    MissingMacroName(Range),
    MissingOperand(Range),
    DuplicateLabel { range: Range, name: SmolStr },
    DuplicateMacroName { range: Range, name: SmolStr },
}

pub struct Label {
    pub section: Section,
    pub statement_index: usize,
}

pub struct MacroDefinition {
    pub statement_index: usize,
}

pub struct Directive {
    pub section: Section,
    pub statement_index: usize,
}

pub struct Instruction {
    pub section: Section,
    pub statement_index: usize,
    pub real_operand_indices: Vec<usize>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Section {
    Bss,
    Data,
    KData,
    KText,
    RData,
    SBss,
    SData,
    Text,
}

fn parse_section(directive_mnemonic: &str) -> Option<Section> {
    match directive_mnemonic {
        ".bss" => Some(Section::Bss),
        ".data" => Some(Section::Data),
        ".kdata" => Some(Section::KData),
        ".ktext" => Some(Section::KText),
        ".rdata" => Some(Section::RData),
        ".sbss" => Some(Section::SBss),
        ".sdata" => Some(Section::SData),
        ".text" => Some(Section::Text),
        _ => None,
    }
}

impl SemanticModel {
    pub fn new() -> SemanticModel {
        SemanticModel {
            syntax_errors: Vec::new(),
            labels: HashMap::new(),
            macros: HashMap::new(),
            directives: Vec::new(),
            instructions: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.syntax_errors.clear();
        self.labels.clear();
        self.macros.clear();
        self.directives.clear();
        self.instructions.clear();
    }

    pub fn parse(&mut self, text: &str, ast: &Ast) {
        self.clear();

        let statements = &ast.items;
        let mut current_section = Section::Text;

        for (statement_index, statement) in statements.iter().enumerate() {
            match statement {
                SyntaxNode::Error(node) => {
                    self.syntax_errors.push(Error::InvalidSyntax(node.range));
                }
                SyntaxNode::Label(node) => {
                    let name = get_text_in_ts_range(text, node.range).to_smolstr();

                    if self.labels.contains_key(&name) {
                        self.syntax_errors.push(Error::DuplicateLabel {
                            range: node.range,
                            name,
                        });
                        continue;
                    }

                    self.labels.insert(
                        name,
                        Label {
                            section: current_section,
                            statement_index,
                        },
                    );
                }
                SyntaxNode::Directive(node) => {
                    let mnemonic = get_text_in_ts_range(text, node.mnemonic.range);
                    if let Some(section) = parse_section(mnemonic) {
                        current_section = section;
                    }

                    self.directives.push(Directive {
                        section: current_section,
                        statement_index,
                    });
                }
                SyntaxNode::Instruction(node) => {
                    let real_operand_indices = node
                        .operands
                        .iter()
                        .enumerate()
                        .filter_map(|(i, operand)| match operand {
                            OperandListItem::Operand(_) => Some(i),
                            OperandListItem::Comma(_) => None,
                            OperandListItem::MissingOperand(r) => {
                                self.syntax_errors.push(Error::MissingOperand(*r));
                                None
                            }
                        })
                        .collect();
                    self.instructions.push(Instruction {
                        section: current_section,
                        statement_index,
                        real_operand_indices,
                    });
                }
                SyntaxNode::MacroInvocation(node) => {
                    let real_operand_indices = node
                        .operands
                        .iter()
                        .enumerate()
                        .filter_map(|(i, operand)| match operand {
                            OperandListItem::Operand(_) => Some(i),
                            OperandListItem::Comma(_) => None,
                            OperandListItem::MissingOperand(r) => {
                                self.syntax_errors.push(Error::MissingOperand(*r));
                                None
                            }
                        })
                        .collect();
                    self.instructions.push(Instruction {
                        section: current_section,
                        statement_index,
                        real_operand_indices,
                    });
                }
                SyntaxNode::MacroDefinition(node) => {
                    let Some(name_node) = &node.name else {
                        self.syntax_errors.push(Error::MissingMacroName(node.range));
                        continue;
                    };
                    let name = get_text_in_ts_range(text, name_node.range).to_smolstr();

                    if name.is_empty() {
                        self.syntax_errors.push(Error::MissingMacroName(node.range));
                        continue;
                    }

                    if self.macros.contains_key(&name) {
                        self.syntax_errors.push(Error::DuplicateMacroName {
                            range: node.range,
                            name,
                        });
                        continue;
                    }

                    self.macros
                        .insert(name, MacroDefinition { statement_index });
                }
            }
        }
    }
}
