// SPDX-License-Identifier: GPL-3.0-only

pub mod vault;

pub use vault::Vault;

pub enum Screen {
    Vault(Vault),
}
