use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub secret: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub totp: String,
}
