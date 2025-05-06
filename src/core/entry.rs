// SPDX-License-Identifier: GPL-3.0-only

use std::fmt::Display;

use anywho::anywho;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: Option<Id>,
    pub name: String,
    pub secret: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub totp: String,
    pub totp_config: TOTPConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub struct Id(pub(crate) Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TOTPConfig {
    pub algorithm: Algorithm,
    pub digits: u32,
    pub skew: u8,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Algorithm {
    #[default]
    Sha1,
    Sha256,
    Sha512,
}

impl Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Algorithm::Sha1 => "SHA1",
            Algorithm::Sha256 => "SHA256",
            Algorithm::Sha512 => "SHA512",
        };
        write!(f, "{}", s)
    }
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
    /// Returns all supported [`Algorithm`]
    pub fn get_all_algorithms() -> Vec<Algorithm> {
        vec![Algorithm::Sha1, Algorithm::Sha256, Algorithm::Sha512]
    }
}

impl Entry {
    /// Generates the TOTP Code of a given [`Entry`]
    pub fn generate_totp(&mut self, refresh_rate: u64) -> Result<(), anywho::Error> {
        use std::time::SystemTime;
        use totp_lite::{Sha1, Sha256, Sha512};

        let cleaned = self
            .secret
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_uppercase();

        let secret = fast32::base32::RFC4648_NOPAD
            .decode(cleaned.as_bytes())
            .map_err(|e| anywho!("Base32 decode error: {:?}", e))?;

        let seconds: u64 = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.totp = match self.totp_config.algorithm {
            Algorithm::Sha1 => {
                totp_lite::totp_custom::<Sha1>(
                    // Calculate a new code every 30 seconds.
                    refresh_rate,
                    // Calculate a 6 digit code.
                    self.totp_config.digits,
                    // Convert the secret into bytes using base32::decode().
                    &secret,
                    // Seconds since the Unix Epoch.
                    seconds,
                )
            }
            Algorithm::Sha256 => totp_lite::totp_custom::<Sha256>(
                refresh_rate,
                self.totp_config.digits,
                &fast32::base32::RFC4648_NOPAD
                    .decode(self.secret.trim().to_uppercase().as_bytes())
                    .unwrap(),
                seconds,
            ),
            Algorithm::Sha512 => totp_lite::totp_custom::<Sha512>(
                refresh_rate,
                self.totp_config.digits,
                &fast32::base32::RFC4648_NOPAD
                    .decode(self.secret.trim().to_uppercase().as_bytes())
                    .unwrap(),
                seconds,
            ),
        };

        Ok(())
    }

    /// Returns true if a [`Entry`] is valid to be submitted.
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

        true
    }
}
