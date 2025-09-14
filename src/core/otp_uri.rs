// SPDX-License-Identifier: GPL-3.0-only

use crate::core::entry::{Algorithm, Entry, TOTPConfig};
use anywho::anywho;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct OtpUri {
    pub label: Option<String>,
    pub issuer: Option<String>,
    pub account_name: Option<String>,
    pub secret: String,
    pub algorithm: Option<String>,
    pub digits: Option<u32>,
}

#[derive(Debug)]
pub enum ParseError {
    InvalidUrl(String),
    InvalidScheme(String),
    InvalidOtpType(String),
    MissingSecret,
    InvalidParameter(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::InvalidUrl(e) => write!(f, "Invalid URL: {}", e),
            ParseError::InvalidScheme(s) => write!(f, "Invalid scheme: {}", s),
            ParseError::InvalidOtpType(t) => write!(f, "Invalid OTP type: {}", t),
            ParseError::MissingSecret => write!(f, "Missing required secret parameter"),
            ParseError::InvalidParameter(p) => write!(f, "Invalid parameter: {}", p),
        }
    }
}

impl std::error::Error for ParseError {}

impl OtpUri {
    pub fn parse(uri: &str) -> Result<Self, ParseError> {
        let url = url::Url::parse(uri).map_err(|e| ParseError::InvalidUrl(e.to_string()))?;

        if url.scheme() != "otpauth" {
            return Err(ParseError::InvalidScheme(url.scheme().to_string()));
        }

        if url.host_str() != Some("totp") {
            return Err(ParseError::InvalidOtpType(
                url.host_str().unwrap_or("missing").to_string(),
            ));
        }

        let mut params: HashMap<String, String> = HashMap::new();
        for (key, value) in url.query_pairs() {
            params.insert(key.to_string(), value.to_string());
        }

        let secret = params
            .get("secret")
            .ok_or(ParseError::MissingSecret)?
            .clone();

        let path = url.path();
        let path = path.strip_prefix('/').unwrap_or(path);

        let (label, account_name, issuer_from_label) = Self::parse_label(path);

        let issuer = params.get("issuer").cloned().or(issuer_from_label);

        let algorithm = params.get("algorithm").cloned();

        let digits = params
            .get("digits")
            .map(|s| s.parse::<u32>())
            .transpose()
            .map_err(|_| ParseError::InvalidParameter("digits".to_string()))?;

        Ok(OtpUri {
            label,
            issuer,
            account_name,
            secret,
            algorithm,
            digits,
        })
    }

    fn parse_label(path: &str) -> (Option<String>, Option<String>, Option<String>) {
        if path.is_empty() {
            return (None, None, None);
        }

        let decoded = urlencoding::decode(path).unwrap_or_else(|_| path.into());

        if let Some(colon_pos) = decoded.find(':') {
            let issuer = decoded[..colon_pos].to_string();
            let account = decoded[colon_pos + 1..].to_string();
            let label = decoded.to_string();

            let issuer = if issuer.is_empty() {
                None
            } else {
                Some(issuer)
            };
            let account = if account.is_empty() {
                None
            } else {
                Some(account)
            };

            (Some(label), account, issuer)
        } else {
            (Some(decoded.to_string()), Some(decoded.to_string()), None)
        }
    }
}

/// Converts an Entry to an OTP URI string
pub fn entry_to_otp_uri(entry: &Entry) -> String {
    let (label, issuer) = parse_entry_name(&entry.name);

    let mut uri = format!("otpauth://totp/{}?", urlencoding::encode(&label));

    uri.push_str("period=30");
    uri.push_str(&format!("&digits={}", entry.totp_config.digits));
    uri.push_str(&format!("&algorithm={}", entry.totp_config.algorithm));
    uri.push_str(&format!("&secret={}", entry.secret));

    if let Some(issuer_name) = issuer {
        uri.push_str(&format!("&issuer={}", urlencoding::encode(&issuer_name)));
    }

    uri
}

/// Parses an entry name to extract label and issuer
/// Remember that the issuer and account name cannot contain a colon (: or %3A)
fn parse_entry_name(name: &str) -> (String, Option<String>) {
    if let Some(colon_pos) = name.find(':') {
        let issuer = name[..colon_pos].trim().to_string();
        let account = name[colon_pos + 1..].trim().to_string();

        // Remove any colons from issuer and account to comply with standard
        let clean_issuer = issuer.replace(':', "").replace("%3A", "");
        let clean_account = account.replace(':', "").replace("%3A", "");

        if clean_issuer.is_empty() {
            (name.to_string(), None)
        } else if clean_account.is_empty() {
            (clean_issuer.clone(), Some(clean_issuer))
        } else {
            (
                format!("{}:{}", clean_issuer, clean_account),
                Some(clean_issuer),
            )
        }
    } else {
        // Clean the name of any colons for single names too
        let clean_name = name.replace(':', "").replace("%3A", "");
        (clean_name, None)
    }
}

/// Converts an OTP URI to an Entry
pub fn otp_uri_to_entry(otp_uri: OtpUri) -> Result<Entry, anywho::Error> {
    let name = otp_uri
        .issuer
        .or(otp_uri.account_name)
        .or(otp_uri.label)
        .unwrap_or_else(|| "Imported Entry".to_string());

    let algorithm = match otp_uri.algorithm.as_deref() {
        Some("SHA1") | None => Algorithm::Sha1,
        Some("SHA256") => Algorithm::Sha256,
        Some("SHA512") => Algorithm::Sha512,
        Some(other) => {
            eprintln!("Warning: Unknown algorithm '{}', defaulting to SHA1", other);
            Algorithm::Sha1
        }
    };

    let digits = otp_uri.digits.unwrap_or(6);
    if !(4..=10).contains(&digits) {
        return Err(anywho!(
            "Invalid digits value: {}, must be between 4 and 10",
            digits
        ));
    }

    let totp_config = TOTPConfig {
        algorithm,
        digits,
        ..Default::default()
    };

    Ok(Entry {
        id: Some(super::entry::Id(Uuid::new_v4())),
        name,
        secret: otp_uri.secret,
        totp: String::new(),
        totp_config,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_to_otp_uri() {
        let entry = Entry {
            id: Some(crate::core::entry::Id(Uuid::new_v4())),
            name: "Google:New Google".to_string(),
            secret: "JBSWY3DPEHPK3PXP".to_string(),
            totp: String::new(),
            totp_config: TOTPConfig::default(),
        };

        let uri = entry_to_otp_uri(&entry);
        assert!(uri.contains("period=30"));
        assert!(uri.contains("digits=6"));
        assert!(uri.contains("algorithm=SHA1"));
        assert!(uri.contains("secret=JBSWY3DPEHPK3PXP"));
        assert!(uri.contains("issuer=Google"));
        assert!(uri.starts_with("otpauth://totp/Google%3ANew%20Google?"));
    }

    #[test]
    fn test_entry_to_otp_uri_no_issuer() {
        let entry = Entry {
            id: Some(crate::core::entry::Id(Uuid::new_v4())),
            name: "Simple Account".to_string(),
            secret: "JBSWY3DPEHPK3PXP".to_string(),
            totp: String::new(),
            totp_config: TOTPConfig {
                algorithm: Algorithm::Sha256,
                digits: 8,
                skew: 1,
            },
        };

        let uri = entry_to_otp_uri(&entry);
        assert!(uri.contains("period=30"));
        assert!(uri.contains("digits=8"));
        assert!(uri.contains("algorithm=SHA256"));
        assert!(!uri.contains("issuer="));
    }

    #[test]
    fn test_otp_uri_to_entry() {
        let otp_uri = OtpUri {
            label: Some("Test Account".to_string()),
            issuer: None,
            account_name: Some("Test Account".to_string()),
            secret: "JBSWY3DPEHPK3PXP".to_string(),
            algorithm: None,
            digits: Some(6),
        };

        let entry = otp_uri_to_entry(otp_uri).unwrap();
        assert_eq!(entry.name, "Test Account");
        assert_eq!(entry.secret, "JBSWY3DPEHPK3PXP");
        assert_eq!(entry.totp_config.digits, 6);
        assert_eq!(entry.totp_config.algorithm, Algorithm::Sha1);
    }
}
