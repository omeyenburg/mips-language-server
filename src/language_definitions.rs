use crate::lang::{Dialect, ISA};
use crate::settings::{Settings, SettingsError};
use serde::{Deserialize, Serialize};
use std::fmt::{format, Debug};
use std::{any, collections::HashMap, collections::HashSet};

use crate::version::{Version, VersionLabel};

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

pub type RawInstructions = HashMap<String, Vec<RawInstructionVariant>>;
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
    pub fn new(settings: &Settings) -> Self {
        let raw_instructions = load_instructions();
        let instructions = process_instructions(raw_instructions, settings)
            .expect("Failed to process instruction definitions");

        let raw_directives = load_directives();
        let directives = process_directives(raw_directives, settings)
            .expect("Failed to process directive definitions");


        let registers = load_registers();

        Self {
            instructions,
            directives,
            registers,
        }
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
        .filter_map(|(mnemonic, raw_variants)| {
            let mut variants = Vec::new();

            for raw in raw_variants {
                let v = match InstructionVariant::try_from(raw) {
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

            let info = build_instruction_hover_info(&mnemonic, &variants, settings);

            Some((mnemonic, Instruction { variants, description: info }))
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

fn build_instruction_hover_info(
    mnemonic: &str,
    variants: &[InstructionVariant],
    settings: &Settings,
) -> String {

    let mut info = format!("{}\n", mnemonic);

    let mut real = String::from("### Variants:\n");
    let mut pseudo  = String::from("### Pseudo Variants:\n");

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
                "```asm\n{} {}\n```\n{}\nIntroduced: {:?}; ",
                mnemonic,
                &v.operands.join(", ").as_str(),
                &v.description,
                &v.introduced
            )
            .as_str(),
        );
        if let Some(d) = v.deprecated {
            part.push_str(format!("Deprecated: {:?}; ", d).as_str());
        }
        part.push_str(
            format!(
                "Assemblers: {}\n\n",
                &v.dialects
                    .iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            )
            .as_str(),
        );
    }

    if has_real {
        info.push_str(&real);
    }

    if has_pseudo && has_pseudo {
        info.push('\n');
    }

    if has_pseudo {
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
