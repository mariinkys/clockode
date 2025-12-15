// SPDX-License-Identifier: GPL-3.0-only

/// Calculates how many seconds remain until the next TOTP refresh
/// based on standard TOTP timing (synchronized to Unix epoch)
///
/// # Arguments
/// * `refresh_rate` - The TOTP refresh interval in seconds (typically 30)
///
/// # Returns
/// The number of seconds until the next refresh occurs
pub fn get_time_until_next_totp_refresh(refresh_rate: u64) -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let seconds: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Calculate remaining seconds until next window
    // This will be synchronized with other TOTP apps since they
    // all count from the same Unix epoch reference point
    refresh_rate - (seconds % refresh_rate)
}
