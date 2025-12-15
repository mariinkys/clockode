// SPDX-License-Identifier: GPL-3.0-only

use crate::app::core::ClockodeEntry;
use anywho::anywho;
use totp_rs::{Algorithm, TOTP};
use uuid::Uuid;

pub const ALL_ALGORITHMS: &[Algorithm] = &[Algorithm::SHA1, Algorithm::SHA256, Algorithm::SHA512];

#[derive(Debug, Clone)]
pub struct InputableClockodeEntry {
    pub uuid: Option<Uuid>,
    pub name: String,
    pub algorithm: Algorithm,
    pub digits: usize,
    pub step: u64,
    pub secret: String,
    pub issuer: Option<String>,
    pub account_name: String,
}

impl Default for InputableClockodeEntry {
    fn default() -> Self {
        Self {
            uuid: Default::default(),
            name: Default::default(),
            algorithm: Default::default(),
            digits: 6,
            step: 30,
            secret: Default::default(),
            issuer: Default::default(),
            account_name: Default::default(),
        }
    }
}

impl From<ClockodeEntry> for InputableClockodeEntry {
    fn from(value: ClockodeEntry) -> Self {
        Self {
            uuid: value.id,
            name: value.name,
            algorithm: value.totp.algorithm,
            digits: value.totp.digits,
            step: value.totp.step,
            secret: value.totp.get_secret_base32(),
            issuer: value.totp.issuer,
            account_name: value.totp.account_name,
        }
    }
}

impl TryFrom<InputableClockodeEntry> for ClockodeEntry {
    type Error = anywho::Error;

    fn try_from(value: InputableClockodeEntry) -> Result<Self, anywho::Error> {
        let secret = value
            .secret
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_uppercase();

        let entry = Self {
            id: value.uuid,
            name: value.name,
            totp: TOTP {
                algorithm: value.algorithm,
                digits: value.digits,
                skew: 0,
                step: value.step,
                secret: totp_rs::Secret::Encoded(secret).to_bytes().map_err(|e| {
                    anywho!("Failed to decode TOTP secret from KeePass entry: {}", e)
                })?,
                issuer: value.issuer,
                account_name: value.account_name,
            },
        };

        Ok(entry)
    }
}

impl TryFrom<String> for InputableClockodeEntry {
    type Error = anywho::Error;

    fn try_from(value: String) -> Result<Self, anywho::Error> {
        let totp = totp_rs::TOTP::from_url_unchecked(value)?;

        Ok(Self {
            uuid: None,
            name: totp.account_name.clone(),
            algorithm: totp.algorithm,
            digits: totp.digits,
            step: totp.step,
            secret: totp.get_secret_base32(),
            issuer: totp.issuer,
            account_name: totp.account_name,
        })
    }
}

impl InputableClockodeEntry {
    /// Returns true if the entry is ready for submission
    pub fn valid(&self) -> bool {
        // Validate name is not empty
        if self.name.trim().is_empty() {
            return false;
        }

        // Validate digits
        if self.digits != 6 && self.digits != 8 {
            return false;
        }

        // Validate period is reasonable (between 1 and 300 seconds)
        if self.step == 0 || self.step > 300 {
            return false;
        }

        // Validate algorithm is one of the supported types
        match self.algorithm {
            Algorithm::SHA1 | Algorithm::SHA256 | Algorithm::SHA512 => {}
        }

        // Validate secret has reasonable length
        // TOTP secrets are typically 16-32 bytes (128-256 bits)
        let secret_len = self.secret.len();
        if !(10..=64).contains(&secret_len) {
            return false;
        }

        // Validate account name is not empty
        if self.account_name.trim().is_empty() {
            return false;
        }

        // Validate issuer does not contain colon
        if self.issuer.as_deref().is_some_and(|x| x.contains(':')) {
            return false;
        }

        true
    }
}
