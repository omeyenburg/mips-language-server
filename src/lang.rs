use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use std::collections::HashMap;
use std::fmt;

use crate::settings::{Settings, SettingsError};
use crate::version::{Version, VersionLabel};

#[derive(Debug, Eq, Serialize, Deserialize, PartialEq)]
pub enum Dialect {
    GAS,
    MARS,
    SPIM,
    Unspecified,
}

impl fmt::Display for Dialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Dialect::GAS => write!(f, "GAS"),
            Dialect::MARS => write!(f, "MARS"),
            Dialect::SPIM => write!(f, "SPIM"),
            Dialect::Unspecified => write!(f, "Unspecified"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum ISA {
    MIPS1,
    MIPS2,
    MIPS3,
    MIPS4,
    MIPS5,
    MIPS32,
    MIPS64,
}

impl Dialect {
    pub fn parse(s: &str) -> Result<Self, SettingsError> {
        match s {
            "gas" => Ok(Dialect::GAS),
            "mars" => Ok(Dialect::MARS),
            "spim" => Ok(Dialect::SPIM),
            "unspecified" => Ok(Dialect::Unspecified),
            _ => Err(SettingsError::UnknownDialect(s.into())),
        }
    }
}

impl ISA {
    pub fn parse(s: &str) -> Result<Self, SettingsError> {
        match s {
            "mips1" => Ok(ISA::MIPS1),
            "mips2" => Ok(ISA::MIPS2),
            "mips3" => Ok(ISA::MIPS3),
            "mips4" => Ok(ISA::MIPS4),
            "mips5" => Ok(ISA::MIPS5),
            "mips32" => Ok(ISA::MIPS32),
            "mips64" => Ok(ISA::MIPS64),
            _ => Err(SettingsError::UnknownISA(s.into())),
        }
    }
}

/*
 *! Instructions
 */
#[derive(Debug, Serialize, Deserialize)]
pub struct RawInstructionVariant {
    pub description: String,
    pub operands: Vec<String>,
    pub dialects: Vec<String>,
    pub introduced: Option<String>,
    pub deprecated: Option<String>,
    pub mips32: bool,
    pub pseudo: bool,
}

impl TryFrom<RawInstructionVariant> for InstructionVariant {
    type Error = SettingsError;

    fn try_from(raw: RawInstructionVariant) -> Result<Self, Self::Error> {
        Ok(Self {
            description: raw.description,
            operands: raw.operands,
            dialects: raw
                .dialects
                .into_iter()
                .map(|d| Dialect::parse(&d))
                .collect::<Result<_, _>>()?,
            introduced: raw
                .introduced
                .as_deref()
                .map(VersionLabel::parse)
                .transpose()?
                .unwrap_or(VersionLabel::Mips1),
            deprecated: raw
                .deprecated
                .as_deref()
                .map(VersionLabel::parse)
                .transpose()?,
            mips32: raw.mips32,
            pseudo: raw.pseudo,
        })
    }
}

#[derive(Debug)]
pub struct Instruction {
    pub variants: Vec<InstructionVariant>,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RawInstruction {
    pub variants: Vec<RawInstructionVariant>,
    pub description: String,
}

pub type RawInstructions = HashMap<String, RawInstruction>;
pub type Instructions = HashMap<String, Instruction>;

/*
 *! Directives
 */
#[derive(Debug, Serialize, Deserialize)]
pub struct RawDirective {
    pub dialects: Vec<String>,
    pub description: String,
}

impl TryFrom<RawDirective> for Directive {
    type Error = SettingsError;

    fn try_from(raw: RawDirective) -> Result<Self, Self::Error> {
        Ok(Self {
            description: raw.description,
            dialects: raw
                .dialects
                .into_iter()
                .map(|d| Dialect::parse(&d))
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug)]
pub struct Directive {
    pub description: String,
    pub dialects: Vec<Dialect>,
}

pub type RawDirectives = HashMap<String, RawDirective>;
pub type Directives = HashMap<String, Directive>;

/*
 *! Registers
 */
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Registers {
    pub numeric: HashMap<String, String>,
    pub common: HashMap<String, String>,
    pub float: HashMap<String, String>,
}

/*
 *! Language Definitions
 */
#[derive(Debug, Default)]
pub struct LanguageDefinitions {
    pub instructions: Instructions,
    pub directives: Directives,
    pub registers: Registers,
}

impl LanguageDefinitions {
    pub fn new() -> Self {
        let instructions = HashMap::new();
        let directives = HashMap::new();
        let registers = load_registers();

        Self {
            instructions,
            directives,
            registers,
        }
    }

    pub fn parse(&mut self, settings: &Settings) {
        let raw_instructions = load_instructions();
        self.instructions = process_instructions(raw_instructions, settings)
            .expect("Failed to process instruction definitions");

        let raw_directives = load_directives();
        self.directives = process_directives(raw_directives, settings)
            .expect("Failed to process directive definitions");

    }
}

fn load_instructions() -> RawInstructions {
    let json = include_str!("../resources/instructions.json");
    serde_json::from_str(json).expect("JSON parsing failed")
}

fn load_directives() -> RawDirectives {
    let json = include_str!("../resources/directives.json");
    serde_json::from_str(json).expect("JSON parsing failed")
}

fn load_registers() -> Registers {
    let json = include_str!("../resources/registers.json");
    serde_json::from_str(json).expect("JSON parsing failed")
}

fn process_instructions(
    raw: RawInstructions,
    settings: &Settings,
) -> Result<Instructions, SettingsError> {
    let instructions = raw
        .into_iter()
        .filter_map(|(mnemonic, raw_instruction)| {
            let mut variants = Vec::new();

            for raw_variants in raw_instruction.variants {
                let v = match InstructionVariant::try_from(raw_variants) {
                    Ok(v) => v,
                    Err(_) => return None,
                };

                if !settings.allows_dialects(&v.dialects) {
                    continue;
                }

                if settings.allow_any_version(&v) || v.is_valid_for(settings.version) {
                    variants.push(v);
                }
            }

            if variants.is_empty() {
                return None;
            }

            let info = build_instruction_hover_info(
                &mnemonic,
                &raw_instruction.description,
                &variants,
                settings,
            );

            Some((
                mnemonic,
                Instruction {
                    variants,
                    description: info,
                },
            ))
        })
        .collect();

    Ok(instructions)
}

fn process_directives(
    raw: RawDirectives,
    settings: &Settings,
) -> Result<Directives, SettingsError> {
    let directives = raw
        .into_iter()
        .filter_map(|(mnemonic, raw_directive)| {
            let d = match Directive::try_from(raw_directive) {
                Ok(d) => d,
                Err(_) => {
                    return None;
                }
            };

            if !settings.allows_dialects(&d.dialects) {
                return None;
            }

            Some((
                mnemonic,
                Directive {
                    description: d.description,
                    dialects: d.dialects,
                },
            ))
        })
        .collect();

    Ok(directives)
}

/// Returns a human-readable description for an instruction with all variants.
///
///  TODO:
///  - refactor
///  - use settings
///  - sort variants by operands
///  - consider merging variants with different imm sizes here
fn build_instruction_hover_info(
    mnemonic: &str,
    base_description: &str,
    variants: &[InstructionVariant],
    settings: &Settings,
) -> String {
    let mut info = format!("{}\n\n", base_description);

    let mut real = String::from("**ISA Variants:**\n");
    let mut pseudo = String::from("**Pseudo Variants:**\n");

    let mut has_real = false;
    let mut has_pseudo = false;
    for v in variants {
        let part = if v.pseudo {
            has_pseudo = true;
            &mut pseudo
        } else {
            has_real = true;
            &mut real
        };

        part.push_str(
            format!(
                "```asm\n{} {}\n```\n{}  \nIntroduced: {:?} | ",
                mnemonic,
                &v.operands.join(", ").as_str(),
                &v.description,
                &v.introduced
            )
            .as_str(),
        );
        if let Some(d) = v.deprecated {
            part.push_str(format!("Deprecated: {:?} | ", d).as_str());
        }

        let dialects_str = {
            let mut dialects: Vec<String> = v.dialects.iter().map(|d| d.to_string()).collect();
            if dialects.is_empty() {
                dialects.push("None".to_string());
            }
            dialects.join(", ")
        };

        part.push_str(format!("Assemblers: {}\n\n", dialects_str).as_str());
    }

    if has_real {
        info.push_str(&real);
    } else {
        info.push_str("**Instruction has no ISA variants.**\n");
    }

    if has_pseudo {
        info.push('\n');
        info.push_str(&pseudo);
    }

    info
}

#[derive(Debug)]
pub struct InstructionVariant {
    pub description: String,
    pub operands: Vec<String>,
    pub dialects: Vec<Dialect>,
    pub introduced: VersionLabel,
    pub deprecated: Option<VersionLabel>,
    pub mips32: bool,
    pub pseudo: bool,
}

impl InstructionVariant {
    fn is_valid_for(&self, target: &Version) -> bool {
        if !target.has_ancestor(self.introduced) {
            return false;
        }

        if let Some(d) = self.deprecated {
            if target.has_ancestor(d) {
                return false;
            }
        }

        true
    }
}
