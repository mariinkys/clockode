// SPDX-License-Identifier: GPL-3.0-only

use anywho::anywho;
use std::path::PathBuf;
use tracing::warn;

use crate::app::core::entry::{ClockodeEntry, import_display_name};

/// Reads a text file with one `otpauth://` URL per line and returns its entries.
pub async fn import(path: PathBuf) -> Result<Vec<ClockodeEntry>, anywho::Error> {
    smol::unblock(move || {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anywho!("Failed to read import file: {}", e))?;

        let entries: Vec<ClockodeEntry> = content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .enumerate()
            .filter_map(|(index, line)| {
                // we use from_url unchecked because of the same reason we can't use TOTP::new
                // Don't use TOTP::new() because it enforces validation and some secrets (ej: microsoft)
                // that are xxxx xxxx xxxx xxxx will fail here if we use ::new() with error:
                // Failed to construct TOTP object: The length of the shared secret MUST be at least 128 bits. 80 bits is not enough
                match totp_rs::Totp::from_url_unchecked(line) {
                    Ok(totp) => {
                        let name = import_display_name(
                            totp.issuer().unwrap_or_default(),
                            totp.account_name(),
                        );

                        Some(ClockodeEntry {
                            id: None,
                            name,
                            totp,
                        })
                    }
                    Err(e) => {
                        warn!("Skipping import entry {}: {}", index + 1, e);
                        None
                    }
                }
            })
            .collect();

        if entries.is_empty() {
            return Err(anywho!("No valid TOTP entries found in the file"));
        }

        Ok(entries)
    })
    .await
}
