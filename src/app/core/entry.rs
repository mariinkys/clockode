use anywho::anywho;
use keepass::db::{Entry, EntryMut, Value};
use totp_rs::{Algorithm, Secret, TOTP};
use tracing::error;
use uuid::Uuid;

// These constants define the names of the custom fields used to store TOTP parameters within a generic KeePass entry.
const CUSTOM_SECRET_KEY: &str = "ClockodeTotpSecret";
const CUSTOM_ALGORITHM_KEY: &str = "ClockodeTotpAlgorithm";
const CUSTOM_PERIOD_KEY: &str = "ClockodeTotpPeriod";
const CUSTOM_DIGITS_KEY: &str = "ClockodeTotpDigits";
const CUSTOM_ISSUER_KEY: &str = "ClockodeTotpIssuer";
const CUSTOM_ACCOUNTNAME_KEY: &str = "ClockodeTotpAccountName";

#[derive(Debug, Clone)]
pub struct ClockodeEntry {
    pub id: Option<Uuid>,
    pub name: String,
    pub totp: TOTP,
}

impl TryFrom<Entry> for ClockodeEntry {
    type Error = anywho::Error;

    fn try_from(value: Entry) -> Result<Self, anywho::Error> {
        let id = value.id().uuid();

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
                error!("Falling back to SHA1 for entry: {}", &name);
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

        let issuer: Option<String> = value.get(CUSTOM_ISSUER_KEY).map(String::from);

        let account_name: String = value
            .get(CUSTOM_ACCOUNTNAME_KEY)
            .unwrap_or(&name)
            .to_string();

        // Don't use TOTP::new() because it enforces validation and some secrets (ej: microsoft)
        // that are xxxx xxxx xxxx xxxx will fail here if we use ::new() with error:
        // Failed to construct TOTP object: The length of the shared secret MUST be at least 128 bits. 80 bits is not enough
        let totp_result = TOTP {
            algorithm,
            digits,
            skew: 0,
            step: period,
            secret: secret_bytes,
            issuer,
            account_name,
        };

        Ok(ClockodeEntry {
            id: Some(id),
            name,
            totp: totp_result,
        })
    }
}

pub fn update_clockode_entry_in_keepass(value: ClockodeEntry, entry: &mut EntryMut) {
    entry
        .fields
        .insert("Title".to_string(), Value::Unprotected(value.name.clone()));

    let secret_b32_string = value.totp.get_secret_base32().to_string();

    entry.fields.insert(
        CUSTOM_SECRET_KEY.to_string(),
        Value::Protected(secrecy::SecretBox::new(Box::new(secret_b32_string))),
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

    entry.fields.insert(
        CUSTOM_ISSUER_KEY.to_string(),
        Value::Unprotected(value.totp.issuer.unwrap_or(value.name)),
    );

    entry.fields.insert(
        CUSTOM_ACCOUNTNAME_KEY.to_string(),
        Value::Unprotected(value.totp.account_name),
    );
}

// impl From<ClockodeEntry> for Entry {
//     fn from(value: ClockodeEntry) -> Self {
//         let mut entry = Entry::new();

//         entry
//             .fields
//             .insert("Title".to_string(), Value::Unprotected(value.name.clone()));

//         let secret_b32_string = value.totp.get_secret_base32().to_string();
//         entry.fields.insert(
//             CUSTOM_SECRET_KEY.to_string(),
//             Value::Protected(secrecy::SecretBox::new(Box::new(secret_b32_string))),
//         );

//         entry.fields.insert(
//             CUSTOM_ALGORITHM_KEY.to_string(),
//             Value::Unprotected(value.totp.algorithm.to_string()),
//         );

//         entry.fields.insert(
//             CUSTOM_PERIOD_KEY.to_string(),
//             Value::Unprotected(value.totp.step.to_string()),
//         );

//         entry.fields.insert(
//             CUSTOM_DIGITS_KEY.to_string(),
//             Value::Unprotected(value.totp.digits.to_string()),
//         );

//         entry.fields.insert(
//             CUSTOM_ISSUER_KEY.to_string(),
//             Value::Unprotected(value.totp.issuer.unwrap_or(value.name)),
//         );

//         entry.fields.insert(
//             CUSTOM_ACCOUNTNAME_KEY.to_string(),
//             Value::Unprotected(value.totp.account_name),
//         );

//         entry
//     }
// }
