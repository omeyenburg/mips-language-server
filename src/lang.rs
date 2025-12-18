use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use std::fmt;

use crate::settings::SettingsError;

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
