pub mod parser;

use tree_sitter::{Point, Range};

#[derive(Debug)]
pub struct Ast {
    pub items: Vec<SyntaxNode>,
    pub range: Range,
}

impl Ast {
    pub fn new() -> Ast {
        Ast {
            items: Vec::with_capacity(0),
            range: Range {
                start_byte: 0,
                end_byte: 0,
                start_point: Point { row: 0, column: 0 },
                end_point: Point { row: 0, column: 0 },
            },
        }
    }
}

#[derive(Debug)]
pub enum SyntaxNode {
    Instruction(InstructionNode),
    MacroInvocation(MacroInvocationNode),
    MacroDefinition(MacroDefinitionNode),
    Directive(DirectiveNode),
    Label(LabelNode),
    Error(SyntaxErrorNode),
}

#[derive(Debug)]
pub struct MacroDefinitionNode {
    pub name: Option<Identifier>,
    pub parameters: Vec<MacroParameterNode>,
    pub range: Range,
}

#[derive(Debug)]
pub struct DirectiveNode {
    pub mnemonic: Identifier,
    pub operands: Vec<OperandListItem>,
    pub range: Range,
}

#[derive(Debug)]
pub enum OperandListItem {
    Operand(ValueNode),
    Comma(Range),
    MissingOperand(Range),
}

#[derive(Debug)]
pub enum ParametersListItem {
    Parameter(Box<MacroParameterNode>),
    Comma(Range),
}

#[derive(Debug)]
pub struct MacroParameterNode {
    pub name: Identifier,
    pub qualifier: Option<Identifier>,
    pub value: Option<ValueNode>,
    pub range: Range,
}

#[derive(Debug)]
pub enum LabelKind {
    Macro,
    Global,
    Local,
    GlobalNumeric,
    LocalNumeric,
}

#[derive(Debug)]
pub struct LabelNode {
    pub kind: LabelKind,
    pub name: Identifier,
    pub range: Range,
}

/// Syntactically an instruction
/// Might be a macro call without parentheses
#[derive(Debug)]
pub struct InstructionNode {
    pub mnemonic: Identifier,
    pub operands: Vec<OperandListItem>,
    pub range: Range,
}

/// Macro call with parentheses
#[derive(Debug)]
pub struct MacroInvocationNode {
    pub mnemonic: Identifier,
    pub operands: Vec<OperandListItem>,
    pub range: Range,
}

#[derive(Debug)]
pub struct Identifier {
    pub range: Range,
}

#[derive(Debug)]
pub enum ValueNode {
    Register {
        range: Range,
    },
    Decimal {
        value: i64,
        range: Range,
    },
    Hexadecimal {
        value: i64,
        range: Range,
    },
    Octal {
        value: i64,
        range: Range,
    },
    Char {
        value: char,
        range: Range,
    },
    Float {
        value: f64,
        range: Range,
    },
    Binary {
        value: i64,
        range: Range,
    },
    Symbol {
        range: Range,
    },
    MacroVariable {
        range: Range,
    },
    String {
        range: Range,
        macro_variables: Vec<ValueNode>,
    },
    OptionFlag {
        range: Range,
    },
    ElfTypeTag {
        range: Range,
    },
    LocalLabelReference {
        range: Range,
    },
    LocalNumericLabelReference {
        range: Range,
    },
    BinaryExpression {
        left: Box<ValueNode>,
        right: Box<ValueNode>,
        operator: OperatorNode,
        range: Range,
    },
    UnaryExpression {
        body: Box<ValueNode>,
        operator: UnaryOperatorNode,
        range: Range,
    },
    /// Might be a simple expression in parentheses
    /// Might be an address with an optional expression as head
    /// Might be parameters of a macro call where head is the macro name
    ParenthesizedExpression {
        head: Option<Box<ValueNode>>,
        body: Vec<OperandListItem>,
        range: Range,
    },
    MalformedValue {
        range: Range,
    },
}

#[derive(Debug)]
pub enum OperatorKind {
    Additive,
    Assignment,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Division,
    Equality,
    LogicalAnd,
    LogicalOr,
    Modulo,
    Multiplicative,
    Relational,
    Shift,
    Subtraction,
}

#[derive(Debug)]
pub enum UnaryOperatorKind {
    Negation,
    BitwiseNegation,
    LogicalNegation,
}

#[derive(Debug)]
pub struct OperatorNode {
    pub kind: OperatorKind,
    pub range: Range,
}

#[derive(Debug)]
pub struct UnaryOperatorNode {
    pub kind: UnaryOperatorKind,
    pub range: Range,
}

#[derive(Debug)]
pub struct SyntaxErrorNode {
    pub range: Range,
}
