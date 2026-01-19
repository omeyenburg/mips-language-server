use crate::settings::SettingsError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VersionLabel {
    Mips1,
    Mips2,
    Mips3,
    Mips4,
    Mips5,
    Mips32r1,
    Mips32r2,
    Mips32r3,
    Mips32r5,
    Mips32r6,
    Mips64r1,
    Mips64r2,
    Mips64r3,
    Mips64r5,
    Mips64r6,
}

impl VersionLabel {
    pub fn parse(s: &str) -> Result<Self, SettingsError> {
        match s {
            "mips1" => Ok(VersionLabel::Mips1),
            "mips2" => Ok(VersionLabel::Mips2),
            "mips3" => Ok(VersionLabel::Mips3),
            "mips4" => Ok(VersionLabel::Mips4),
            "mips5" => Ok(VersionLabel::Mips5),
            "mips32r1" => Ok(VersionLabel::Mips32r1),
            "mips32r2" => Ok(VersionLabel::Mips32r2),
            "mips32r3" => Ok(VersionLabel::Mips32r3),
            "mips32r5" => Ok(VersionLabel::Mips32r5),
            "mips32r6" => Ok(VersionLabel::Mips32r6),
            "mips64r1" => Ok(VersionLabel::Mips64r1),
            "mips64r2" => Ok(VersionLabel::Mips64r2),
            "mips64r3" => Ok(VersionLabel::Mips64r3),
            "mips64r5" => Ok(VersionLabel::Mips64r5),
            "mips64r6" => Ok(VersionLabel::Mips64r6),
            _ => Err(SettingsError::UnknownVersion(s.into())),
        }
    }
}

#[derive(Debug)]
pub struct Version {
    label: VersionLabel,
    ancestors: &'static [VersionLabel],
}

pub static MIPS1: Version = Version {
    label: VersionLabel::Mips1,
    ancestors: &[VersionLabel::Mips1],
};

pub static MIPS2: Version = Version {
    label: VersionLabel::Mips2,
    ancestors: &[VersionLabel::Mips1, VersionLabel::Mips2],
};

pub static MIPS3: Version = Version {
    label: VersionLabel::Mips3,
    ancestors: &[
        VersionLabel::Mips1,
        VersionLabel::Mips2,
        VersionLabel::Mips3,
    ],
};

pub static MIPS4: Version = Version {
    label: VersionLabel::Mips4,
    ancestors: &[
        VersionLabel::Mips1,
        VersionLabel::Mips2,
        VersionLabel::Mips3,
        VersionLabel::Mips4,
    ],
};

pub static MIPS5: Version = Version {
    label: VersionLabel::Mips5,
    ancestors: &[
        VersionLabel::Mips1,
        VersionLabel::Mips2,
        VersionLabel::Mips3,
        VersionLabel::Mips4,
        VersionLabel::Mips5,
    ],
};

pub static MIPS32R1: Version = Version {
    label: VersionLabel::Mips32r1,
    ancestors: &[
        VersionLabel::Mips1,
        VersionLabel::Mips2,
        VersionLabel::Mips32r1,
    ],
};

pub static MIPS32R2: Version = Version {
    label: VersionLabel::Mips32r2,
    ancestors: &[
        VersionLabel::Mips1,
        VersionLabel::Mips2,
        VersionLabel::Mips32r1,
        VersionLabel::Mips32r2,
    ],
};

pub static MIPS32R3: Version = Version {
    label: VersionLabel::Mips32r3,
    ancestors: &[
        VersionLabel::Mips1,
        VersionLabel::Mips2,
        VersionLabel::Mips32r1,
        VersionLabel::Mips32r2,
        VersionLabel::Mips32r3,
    ],
};

pub static MIPS32R5: Version = Version {
    label: VersionLabel::Mips32r5,
    ancestors: &[
        VersionLabel::Mips1,
        VersionLabel::Mips2,
        VersionLabel::Mips32r1,
        VersionLabel::Mips32r2,
        VersionLabel::Mips32r3,
        VersionLabel::Mips32r5,
    ],
};

pub static MIPS32R6: Version = Version {
    label: VersionLabel::Mips32r6,
    ancestors: &[
        VersionLabel::Mips1,
        VersionLabel::Mips2,
        VersionLabel::Mips32r1,
        VersionLabel::Mips32r2,
        VersionLabel::Mips32r3,
        VersionLabel::Mips32r5,
        VersionLabel::Mips32r6,
    ],
};

pub static MIPS64R1: Version = Version {
    label: VersionLabel::Mips64r1,
    ancestors: &[
        VersionLabel::Mips1,
        VersionLabel::Mips2,
        VersionLabel::Mips3,
        VersionLabel::Mips4,
        VersionLabel::Mips5,
        VersionLabel::Mips32r1,
        VersionLabel::Mips64r1,
    ],
};

pub static MIPS64R2: Version = Version {
    label: VersionLabel::Mips64r2,
    ancestors: &[
        VersionLabel::Mips1,
        VersionLabel::Mips2,
        VersionLabel::Mips3,
        VersionLabel::Mips4,
        VersionLabel::Mips5,
        VersionLabel::Mips32r1,
        VersionLabel::Mips32r2,
        VersionLabel::Mips64r1,
        VersionLabel::Mips64r2,
    ],
};

pub static MIPS64R3: Version = Version {
    label: VersionLabel::Mips64r3,
    ancestors: &[
        VersionLabel::Mips1,
        VersionLabel::Mips2,
        VersionLabel::Mips3,
        VersionLabel::Mips4,
        VersionLabel::Mips5,
        VersionLabel::Mips32r1,
        VersionLabel::Mips32r2,
        VersionLabel::Mips32r3,
        VersionLabel::Mips64r1,
        VersionLabel::Mips64r2,
        VersionLabel::Mips64r3,
    ],
};

pub static MIPS64R5: Version = Version {
    label: VersionLabel::Mips64r5,
    ancestors: &[
        VersionLabel::Mips1,
        VersionLabel::Mips2,
        VersionLabel::Mips3,
        VersionLabel::Mips4,
        VersionLabel::Mips5,
        VersionLabel::Mips32r1,
        VersionLabel::Mips32r2,
        VersionLabel::Mips32r3,
        VersionLabel::Mips32r5,
        VersionLabel::Mips64r1,
        VersionLabel::Mips64r2,
        VersionLabel::Mips64r3,
        VersionLabel::Mips64r5,
    ],
};

pub static MIPS64R6: Version = Version {
    label: VersionLabel::Mips64r6,
    ancestors: &[
        VersionLabel::Mips1,
        VersionLabel::Mips2,
        VersionLabel::Mips3,
        VersionLabel::Mips4,
        VersionLabel::Mips5,
        VersionLabel::Mips32r1,
        VersionLabel::Mips32r2,
        VersionLabel::Mips32r3,
        VersionLabel::Mips32r5,
        VersionLabel::Mips32r6,
        VersionLabel::Mips64r1,
        VersionLabel::Mips64r2,
        VersionLabel::Mips64r3,
        VersionLabel::Mips64r5,
        VersionLabel::Mips64r6,
    ],
};

/*
Version inheritance tree:
    mips1
    └── mips2
        │               ┌── mips32r6 ───────────── mips64r6
        │           ┌── mips32r5 ───────────── mips64r5 ─┘
        │       ┌── mips32r3 ───────────── mips64r3 ─┘
        │   ┌── mips32r2 ───────────── mips64r2 ─┘
        ├── mips32r1 ───────────── mips64r1 ─┘
        │           ┌── mips5 ─────┘
        │       ┌── mips4
        └── mips3
*/
impl Version {
    pub fn has_ancestor(&self, ancestor: VersionLabel) -> bool {
        self.ancestors.contains(&ancestor)
    }

    pub fn parse(s: &str) -> Result<&'static Version, SettingsError> {
        let unformatted: String = s.to_lowercase().replace(" ", "");

        match unformatted.as_str() {
            "mips1" => Ok(&MIPS1),
            "mipsi" => Ok(&MIPS1),
            "mips2" => Ok(&MIPS2),
            "mipsii" => Ok(&MIPS2),
            "mips3" => Ok(&MIPS3),
            "mipsiii" => Ok(&MIPS3),
            "mips4" => Ok(&MIPS4),
            "mipsiv" => Ok(&MIPS4),
            "mips5" => Ok(&MIPS5),
            "mipsv" => Ok(&MIPS5),
            "mips32r1" => Ok(&MIPS32R1),
            "mips32r2" => Ok(&MIPS32R2),
            "mips32r3" => Ok(&MIPS32R3),
            "mips32r5" => Ok(&MIPS32R5),
            "mips32r6" => Ok(&MIPS32R6),
            "mips64r1" => Ok(&MIPS64R1),
            "mips64r2" => Ok(&MIPS64R2),
            "mips64r3" => Ok(&MIPS64R3),
            "mips64r5" => Ok(&MIPS64R5),
            "mips64r6" => Ok(&MIPS64R6),
            _ => Err(SettingsError::UnknownVersion(s.into())),
        }
    }
}
