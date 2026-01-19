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
    version: Option<String>,
}

#[derive(Debug)]
pub struct Settings {
    pub dialect: Dialect,
    pub version: &'static version::Version,
}

impl Settings {
    pub fn default() -> Self {
        Settings {
            dialect: Dialect::Unspecified,
            version: &version::MIPS64R5,
        }
    }
    // pub fn new(options: Option<Value>) -> Result<Self, SettingsError> {
    //     let mut settings = Self {
    //         dialect: Dialect::Unspecified,
    //         version: &version::MIPS64R6,
    //     };

    //     let raw = options
    //         .map(serde_json::from_value::<RawSettings>)
    //         .transpose()
    //         .map_err(SettingsError::from)?
    //         .unwrap_or_default();

    //     if let Some(d) = raw.dialect.as_deref() {
    //         settings.dialect = Dialect::parse(d)?;
    //     };

    //     Ok(settings)
    // }

    pub fn parse(&mut self, unparsed_settings: Value) -> Result<(), SettingsError> {
        // let raw_settings = unparsed_settings
        //     .map()
        //     .transpose()
        //     .map_err(SettingsError::from)?
        //     .unwrap_or_default();

        let raw_settings = serde_json::from_value::<RawSettings>(unparsed_settings)
            .map_err(SettingsError::from)?;

        if let Some(d) = raw_settings.dialect.as_deref() {
            self.dialect = Dialect::parse(d)?;
        };

        if let Some(v) = raw_settings.version.as_deref() {
            self.version = version::Version::parse(v)?;
        };

        Ok(())
    }

    pub fn allows_dialects(&self, dialects: &[Dialect]) -> bool {
        self.dialect == Dialect::Unspecified || dialects.contains(&self.dialect)
    }

    // Requires that dialect is supported
    pub fn allow_any_version(&self, v: &InstructionVariant) -> bool {
        self.dialect == Dialect::MARS || self.dialect == Dialect::SPIM
    }
}
