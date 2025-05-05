use serde::{Deserialize, Serialize};
use totp_rs::Algorithm;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub secret: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub totp: String,
    pub totp_config: TOTPConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TOTPConfig {
    pub algorithm: Algorithm,
    pub digits: usize,
    pub skew: u8,
}

impl Default for TOTPConfig {
    fn default() -> Self {
        Self {
            algorithm: Default::default(),
            digits: 6,
            skew: 1,
        }
    }
}

impl TOTPConfig {
    pub fn get_all_algorithms() -> Vec<Algorithm> {
        vec![Algorithm::SHA1, Algorithm::SHA256, Algorithm::SHA512]
    }
}

impl Entry {
    pub fn is_valid(&self) -> bool {
        if self.name.is_empty() || self.secret.is_empty() {
            return false;
        }

        if self.totp_config.digits < 1 {
            return false;
        }

        if self.totp_config.skew == 0 {
            return false;
        }

        return true;
    }
}
