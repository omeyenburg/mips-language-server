use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use std::fmt;

use crate::lang::{Dialect, InstructionVariant, ISA};
use crate::version;

#[derive(Debug)]
pub enum SettingsError {
    UnknownDialect(String),
    UnknownISA(String),
    UnknownVersion(String),
    InvalidRevision(u32),
    InvalidSyntax,
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::UnknownDialect(d) => write!(f, "unknown dialect: `{}`", d),
            SettingsError::UnknownISA(i) => write!(f, "unknown ISA: `{}`", i),
            SettingsError::UnknownVersion(i) => write!(f, "unknown version: `{}`", i),
            SettingsError::InvalidRevision(r) => write!(f, "invalid revision: {}", r),
            SettingsError::InvalidSyntax => write!(f, "failed to parse initialization options"),
        }
    }
}

impl From<serde_json::Error> for SettingsError {
    fn from(_: serde_json::Error) -> Self {
        SettingsError::InvalidSyntax
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct RawSettings {
    dialect: Option<String>,
    isa: Option<String>,
    revision: Option<u32>,
}

#[derive(Debug)]
pub struct Settings {
    pub dialect: Dialect,
    // isa: ISA,
    // revision: u32,
    pub version: &'static version::Version,
}

impl Settings {
    pub fn new(options: Option<Value>) -> Result<Self, SettingsError> {
        let mut settings = Self {
            dialect: Dialect::Unspecified,
            // isa: ISA::MIPS64,
            // revision: 6,
            version: &version::MIPS64R6,
        };

        let raw = options
            .map(serde_json::from_value::<RawSettings>)
            .transpose()
            .map_err(SettingsError::from)?
            .unwrap_or_default();

        if let Some(d) = raw.dialect.as_deref() {
            settings.dialect = Dialect::parse(d)?;
        };

        // if let Some(i) = raw.isa.as_deref() {
        //     self.isa = ISA::parse(i)?
        // }

        // match raw.revision {
        //     Some(r @ (1 | 2 | 3 | 5 | 6)) => self.revision = r,
        //     Some(r) => return Err(SettingsError::InvalidRevision(r)),
        //     _ => (),
        // };

        Ok(settings)
    }

    pub fn allows_dialects(&self, dialects: &[Dialect]) -> bool {
        self.dialect == Dialect::Unspecified || dialects.contains(&self.dialect)
    }

    // Requires that dialect is supported
    pub fn allow_any_version(&self, v: &InstructionVariant) -> bool {
        self.dialect == Dialect::MARS || self.dialect == Dialect::SPIM
    }
}
