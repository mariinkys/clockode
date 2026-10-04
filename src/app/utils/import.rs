use std::fmt::Display;

#[derive(Debug, Clone)]
pub enum ImportType {
    Standard,
    AegisEncrypted,
}

impl ImportType {
    /// A list with all the import types.
    pub const ALL: &'static [Self] = &[Self::Standard, Self::AegisEncrypted];
}

impl Display for ImportType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            ImportType::Standard => write!(f, "Standard"),
            ImportType::AegisEncrypted => write!(f, "Aegis Encypted"),
        }
    }
}
