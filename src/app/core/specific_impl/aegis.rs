// SPDX-License-Identifier: GPL-3.0-only

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use anywho::anywho;
use base64::{Engine, engine::general_purpose::STANDARD};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::path::PathBuf;
use tracing::warn;

use crate::app::core::entry::ClockodeEntry;

#[derive(Deserialize)]
struct Vault {
    header: Header,
    db: serde_json::Value,
}

#[derive(Deserialize)]
struct Header {
    slots: Option<Vec<Slot>>,
    params: Option<KeyParams>,
}

#[derive(Deserialize)]
struct Slot {
    /// 1 = password slot, 2 = biometric
    #[serde(rename = "type")]
    kind: u8,
    key: String,
    key_params: KeyParams,
    n: Option<u32>,
    r: Option<u32>,
    p: Option<u32>,
    salt: Option<String>,
}

#[derive(Deserialize)]
struct KeyParams {
    nonce: String,
    tag: String,
}

#[derive(Deserialize)]
struct Db {
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    #[serde(rename = "type")]
    kind: String,
    name: String,
    issuer: Option<String>,
    info: Info,
}

#[derive(Deserialize)]
struct Info {
    secret: String,
    algo: Option<String>,
    digits: Option<u32>,
    period: Option<u64>,
}

/// Reads an Aegis vault (encrypted or plain) and returns its TOTP entries.
pub async fn import(
    path: PathBuf,
    password: SecretString,
) -> Result<Vec<ClockodeEntry>, anywho::Error> {
    smol::unblock(move || {
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| anywho!("Failed to read import file: {}", e))?;
        let vault: Vault =
            serde_json::from_str(&raw).map_err(|e| anywho!("Not a valid Aegis vault: {}", e))?;

        let db: Db = match (vault.header.slots, vault.header.params) {
            (Some(slots), Some(params)) => {
                let key = master_key(&slots, &password)?;
                let ciphertext = STANDARD
                    .decode(
                        vault
                            .db
                            .as_str()
                            .ok_or_else(|| anywho!("Malformed vault"))?,
                    )
                    .map_err(|e| anywho!("Malformed vault: {}", e))?;
                let plain = decrypt(&key, &params, &ciphertext)
                    .map_err(|_| anywho!("Vault contents could not be decrypted"))?;

                serde_json::from_slice(&plain)
                    .map_err(|e| anywho!("Malformed vault contents: {}", e))?
            }
            // Unencrypted export
            _ => serde_json::from_value(vault.db)
                .map_err(|e| anywho!("Malformed vault contents: {}", e))?,
        };

        let entries: Vec<_> = db.entries.into_iter().filter_map(to_entry).collect();

        if entries.is_empty() {
            return Err(anywho!("No TOTP entries found in the vault"));
        }

        Ok(entries)
    })
    .await
}

/// Tries the password against every password slot.
fn master_key(slots: &[Slot], password: &SecretString) -> Result<Vec<u8>, anywho::Error> {
    for slot in slots.iter().filter(|slot| slot.kind == 1) {
        let (Some(n), Some(r), Some(p), Some(salt)) = (slot.n, slot.r, slot.p, &slot.salt) else {
            continue;
        };

        let (Ok(salt), Ok(wrapped)) = (hex_decode(salt), hex_decode(&slot.key)) else {
            continue;
        };
        let Ok(params) = scrypt::Params::new(n.max(2).ilog2() as u8, r, p) else {
            continue;
        };

        let mut derived = [0u8; 32];
        if scrypt::scrypt(
            password.expose_secret().as_bytes(),
            &salt,
            &params,
            &mut derived,
        )
        .is_err()
        {
            continue;
        }

        if let Ok(key) = decrypt(&derived, &slot.key_params, &wrapped) {
            return Ok(key);
        }
    }

    Err(anywho!("Incorrect Password"))
}

fn decrypt(key: &[u8], params: &KeyParams, ciphertext: &[u8]) -> Result<Vec<u8>, anywho::Error> {
    let nonce: [u8; 12] = hex_decode(&params.nonce)
        .map_err(|e| anywho!("Bad nonce: {}", e))?
        .try_into()
        .map_err(|_| anywho!("Bad nonce length"))?;
    let tag = hex_decode(&params.tag).map_err(|e| anywho!("Bad tag: {}", e))?;

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| anywho!("Bad key length"))?;

    // Aegis stores the tag separately, aes-gcm wants it appended
    let mut data = ciphertext.to_vec();
    data.extend_from_slice(&tag);

    cipher
        .decrypt(&Nonce::from(nonce), data.as_ref())
        .map_err(|_| anywho!("Decryption failed"))
}

fn to_entry(entry: Entry) -> Option<ClockodeEntry> {
    if entry.kind != "totp" {
        warn!("Skipping unsupported Aegis entry type '{}'", entry.kind);
        return None;
    }

    let issuer = entry.issuer.unwrap_or_default();
    let account = if entry.name.trim().is_empty() {
        &issuer
    } else {
        &entry.name
    };

    // Same path as the standard import, so the same secret-length leniency applies
    let mut url = format!(
        "otpauth://totp/{}?secret={}&algorithm={}&digits={}&period={}",
        percent_encode(account),
        entry.info.secret,
        entry.info.algo.as_deref().unwrap_or("SHA1"),
        entry.info.digits.unwrap_or(6),
        entry.info.period.unwrap_or(30),
    );
    if !issuer.is_empty() {
        url.push_str(&format!("&issuer={}", percent_encode(&issuer)));
    }

    let totp = match totp_rs::Totp::from_url_unchecked(&url) {
        Ok(totp) => totp,
        Err(e) => {
            warn!("Skipping Aegis entry '{}': {}", entry.name, e);
            return None;
        }
    };

    let name = match (issuer.trim(), entry.name.trim()) {
        ("", "") => "Default".to_string(),
        ("", name) => name.to_string(),
        (issuer, "") => issuer.to_string(),
        (issuer, name) => format!("{issuer} ({name})"),
    };

    Some(ClockodeEntry {
        id: None,
        name,
        totp,
    })
}

/// Decodes a hex string into bytes.
fn hex_decode(s: &str) -> Result<Vec<u8>, anywho::Error> {
    if !s.is_ascii() || !s.len().is_multiple_of(2) {
        return Err(anywho!("Invalid hex string"));
    }

    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| anywho!("Invalid hex string")))
        .collect()
}

/// Percent-encodes everything except unreserved URL characters.
fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
