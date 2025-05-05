use anywho::anywho;
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
    pub fn generate_totp(&mut self, refresh_rate: u64) -> Result<(), anywho::Error> {
        use std::time::SystemTime;
        use totp_lite::Sha1;

        let length = self.secret.trim().len();
        if length != 16 && length != 26 && length != 32 {
            return Err(anywho!(
                "Invalid TOTP secret, must be 16, 26 or 32 characters."
            ));
        }

        let seconds: u64 = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.totp = totp_lite::totp_custom::<Sha1>(
            // Calculate a new code every 30 seconds.
            refresh_rate,
            // Calculate a 6 digit code.
            self.totp_config.digits.try_into().unwrap(),
            // Convert the secret into bytes using base32::decode().
            &fast32::base32::RFC4648_NOPAD
                .decode(self.secret.trim().to_uppercase().as_bytes())
                .unwrap(),
            // Seconds since the Unix Epoch.
            seconds,
        );

        Ok(())
    }

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
