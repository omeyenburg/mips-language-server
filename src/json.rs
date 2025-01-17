use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/*
 *! Instructions
 */
#[derive(Debug, Serialize, Deserialize)]
pub struct InstructionVariant {
    pub operands: Vec<String>, // List of operand types
    pub description: String,   // Description of this variant
    pub code: String,          // Machine code
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Instruction {
    pub format: String,                    // Type of instruction, e.g. Register-Type
    pub variants: Vec<InstructionVariant>, // List of all variants
}
pub type Instructions = HashMap<String, Instruction>;
pub fn read_instructions() -> Instructions {
    let json = include_str!("../resources/instructions.json");
    serde_json::from_str(json).expect("JSON parsing failed")
}

/*
 *! Pseudo Instructions
 *
 * we need also to somehow handle the COMPACT thingy
 * would be again redundant but easy, to split all COMPACTS
 * and gegenrate an extra description
 */
#[derive(Debug, Serialize, Deserialize)]
pub struct PseudoInstructionVariant {
    pub operands: Vec<String>,    // List of operand types
    pub replacement: Vec<String>, // List of instructions that would be substituted
    pub descriptions: String,     // Description of this variant
}

// Format is always: Pseudo-Instruction, no need to store this information
pub type PseudoInstruction = Vec<PseudoInstructionVariant>;
pub type PseudoInstructions = HashMap<String, Vec<PseudoInstruction>>;
pub fn read_pseudo_instructions() -> PseudoInstructions {
    let json = include_str!("../resources/pseudo-instructions.json");
    serde_json::from_str(json).expect("JSON parsing failed")
}

/*
 *! Directives
 */
pub type Directives = HashMap<String, String>;
pub fn read_directives() -> Directives {
    let json = include_str!("../resources/directives.json");
    serde_json::from_str(json).expect("JSON parsing failed")
}

/*
 *! Registers
 */
#[derive(Debug, Serialize, Deserialize)]
pub struct Registers {
    pub numeric: HashMap<String, String>,
    pub common: HashMap<String, String>,
    pub float: HashMap<String, String>,
}
pub fn read_registers() -> Registers {
    let json = include_str!("../resources/registers.json");
    serde_json::from_str(json).expect("JSON parsing failed")
}
