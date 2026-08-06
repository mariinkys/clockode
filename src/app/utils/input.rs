// SPDX-License-Identifier: GPL-3.0-only

use crate::app::core::ClockodeEntry;
use anywho::anywho;
use totp_rs::{Algorithm, Secret};
use uuid::Uuid;

pub const ALL_ALGORITHMS: &[Algorithm] = &[Algorithm::SHA1, Algorithm::SHA256, Algorithm::SHA512];

#[derive(Debug, Clone)]
pub struct InputableClockodeEntry {
    pub uuid: Option<Uuid>,
    pub name: String,
    pub algorithm: Algorithm,
    pub digits: u8,
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
            algorithm: value.totp.algorithm(),
            digits: value.totp.digits(),
            step: value.totp.step(),
            secret: value.totp.secret().to_base32(),
            issuer: value.totp.issuer().map(ToOwned::to_owned),
            account_name: value.totp.account_name().to_string(),
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

        let secret = Secret::try_from_base32(secret)
            .map_err(|e| anywho!("Failed to decode TOTP secret from KeePass entry: {}", e))?;
        let secret_bytes: &[u8] = secret.as_bytes();

        let entry = Self {
            id: value.uuid,
            name: value.name,
            totp: totp_rs::Builder::new().with_algorithm(value.algorithm).with_digits(value.digits).with_skew(0).with_step_duration(value.step).with_secret(secret_bytes).with_issuer(value.issuer).with_account_name(value.account_name).build_noncompliant()
        };

        Ok(entry)
    }
}

impl TryFrom<String> for InputableClockodeEntry {
    type Error = anywho::Error;

    fn try_from(value: String) -> Result<Self, anywho::Error> {
        let totp = totp_rs::Totp::from_url_unchecked(value)?;

        Ok(Self {
            uuid: None,
            name: totp.account_name().to_string(),
            algorithm: totp.algorithm(),
            digits: totp.digits(),
            step: totp.step(),
            secret: totp.secret().to_base32(),
            issuer: totp.issuer().map(ToOwned::to_owned),
            account_name: totp.account_name().to_string(),
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
            _ => return false,
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

    pub fn get_qr_bytes(&self) -> Result<Vec<u8>, anywho::Error> {
        if !self.valid() {
            return Err(anywho!("Invalid Entity"));
        };

        let secret = Secret::try_from_base32(self.secret.clone())
            .map_err(|e| anywho!("Failed to decode TOTP secret from KeePass entry: {}", e))?;
        let secret_bytes: &[u8] = secret.as_bytes();

        let totp = totp_rs::Builder::new().with_algorithm(self.algorithm).with_digits(self.digits).with_skew(0).with_step_duration(self.step).with_secret(secret_bytes).with_issuer(self.issuer.clone()).with_account_name(self.account_name.clone()).build_noncompliant();


        let qr = totp
            .to_qr_png()
            .map_err(|e| anywho!("Error generating the QR Code: {}", e))?;

        Ok(qr)
    }
}
