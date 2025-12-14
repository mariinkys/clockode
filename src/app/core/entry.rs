use anywho::anywho;
use keepass::db::{Entry, Value};
use totp_rs::{Algorithm, Secret, TOTP};

// These constants define the names of the custom fields used to store TOTP parameters within a generic KeePass entry.
const CUSTOM_SECRET_KEY: &str = "ClockodeTotpSecret";
const CUSTOM_ALGORITHM_KEY: &str = "ClockodeTotpAlgorithm";
const CUSTOM_PERIOD_KEY: &str = "ClockodeTotpPeriod";
const CUSTOM_DIGITS_KEY: &str = "ClockodeTotpDigits";

#[derive(Debug, Clone)]
pub struct ClockodeEntry {
    pub name: String,
    pub totp: TOTP,
}

impl ClockodeEntry {
    /// Returns true if the entry is ready for submission
    pub fn valid(&self) -> bool {
        // Validate name is not empty
        if self.name.trim().is_empty() {
            return false;
        }

        // Validate digits
        if self.totp.digits != 6 && self.totp.digits != 8 {
            return false;
        }

        // Validate period is reasonable (between 1 and 300 seconds)
        if self.totp.step == 0 || self.totp.step > 300 {
            return false;
        }

        // Validate algorithm is one of the supported types
        match self.totp.algorithm {
            Algorithm::SHA1 | Algorithm::SHA256 | Algorithm::SHA512 => {}
        }

        // Validate secret has reasonable length
        // TOTP secrets are typically 16-32 bytes (128-256 bits)
        let secret_len = self.totp.secret.len();
        if !(10..=64).contains(&secret_len) {
            return false;
        }

        true
    }
}

impl TryFrom<Entry> for ClockodeEntry {
    type Error = anywho::Error;

    fn try_from(value: Entry) -> Result<Self, anywho::Error> {
        let name = value
            .get_title()
            .unwrap_or("Unnamed TOTP Entry")
            .to_string();

        let secret_encoded_str = value
            .get(CUSTOM_SECRET_KEY)
            .ok_or_else(|| anywho!("Missing TOTP secret in KeePass entry"))?
            .to_string();

        let algorithm_str = value.get(CUSTOM_ALGORITHM_KEY).unwrap_or("SHA1");
        let algorithm: Algorithm = match algorithm_str.to_uppercase().as_str() {
            "SHA1" => Algorithm::SHA1,
            "SHA256" => Algorithm::SHA256,
            "SHA512" => Algorithm::SHA512,
            _ => {
                eprintln!("Falling back to SHA1 for entry: {}", &name);
                Algorithm::SHA1
            }
        };

        let digits: usize = value
            .get(CUSTOM_DIGITS_KEY)
            .and_then(|s| s.parse().ok())
            .unwrap_or(6);

        let period: u64 = value
            .get(CUSTOM_PERIOD_KEY)
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);

        let secret_bytes: Vec<u8> = Secret::Encoded(secret_encoded_str)
            .to_bytes()
            .map_err(|e| anywho!("Failed to decode TOTP secret from KeePass entry: {}", e))?;

        let totp_result = TOTP::new(algorithm, digits, 0, period, secret_bytes)
            .map_err(|e| anywho!("Failed to construct TOTP object: {}", e))?;

        Ok(ClockodeEntry {
            name,
            totp: totp_result,
        })
    }
}

impl From<ClockodeEntry> for Entry {
    fn from(value: ClockodeEntry) -> Self {
        let mut entry = Entry::new();

        entry
            .fields
            .insert("Title".to_string(), Value::Unprotected(value.name));

        let secret_b32_string = value.totp.get_secret_base32().to_string();
        entry.fields.insert(
            CUSTOM_SECRET_KEY.to_string(),
            Value::Protected(secret_b32_string.into()),
        );

        entry.fields.insert(
            CUSTOM_ALGORITHM_KEY.to_string(),
            Value::Unprotected(value.totp.algorithm.to_string()),
        );

        entry.fields.insert(
            CUSTOM_PERIOD_KEY.to_string(),
            Value::Unprotected(value.totp.step.to_string()),
        );

        entry.fields.insert(
            CUSTOM_DIGITS_KEY.to_string(),
            Value::Unprotected(value.totp.digits.to_string()),
        );

        entry
    }
}
