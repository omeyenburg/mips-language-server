use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Instruction {
    pub syntax: String,
    pub description: String,
    pub format: String,
    pub code: String,
}

pub type Instructions = std::collections::HashMap<String, Vec<Instruction>>;
pub type Directives = std::collections::HashMap<String, String>;
pub type Registers = std::collections::HashMap<String, String>;

pub fn read_instructions() -> Instructions {
    let json = include_str!("../resources/instructions.json");
    serde_json::from_str(json).expect("JSON parsing failed")
}

pub fn read_directives() -> Directives {
    let json = include_str!("../resources/directives.json");
    serde_json::from_str(json).expect("JSON parsing failed")
}

pub fn read_registers() -> Registers {
    let json = include_str!("../resources/registers.json");
    serde_json::from_str(json).expect("JSON parsing failed")
}
