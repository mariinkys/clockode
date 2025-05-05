use super::entry::Entry;
use aes_gcm::aead::rand_core::RngCore;
use anywho::anywho;
use ron::{de, ser};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};
use uuid::Uuid;

const APP_ID: &str = "dev.mariinkys.IcedTwoFA";

#[derive(Debug, Clone)]
pub struct Vault {
    path: Box<Path>,
    state: State,
}

#[derive(Debug, Clone)]
enum State {
    Locked,
    Unlocked {
        data: VaultData,
        encryption_key: Vec<u8>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct EncryptedVault {
    salt: String,
    nonce: Vec<u8>,
    encrypted_data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VaultData {
    entries: HashMap<super::entry::Id, Entry>,
}

impl Vault {
    /// Attempts to load an existing [`Vault`] on its default path
    pub async fn load() -> Result<Self, anywho::Error> {
        use dirs;
        use tokio::task;

        task::spawn_blocking(|| {
            let vault_path = dirs::data_dir()
                .ok_or_else(|| anywho!("Could not determine data directory"))?
                .join(APP_ID)
                .join("vault");

            if vault_path.exists() {
                Ok(Self {
                    path: Box::from(vault_path),
                    state: State::Locked,
                })
            } else {
                eprintln!("Vault not found on {:?}", vault_path);
                Err(anywho!("Vault not found"))
            }
        })
        .await?
    }

    /// Attempts to decrypt a [`Vault`] given a password
    pub async fn decrypt(password: String, mut vault: Self) -> Result<Self, anywho::Error> {
        use aes_gcm::{
            Aes256Gcm, Nonce,
            aead::{Aead, KeyInit, Payload},
        };
        use scrypt::{
            Scrypt,
            password_hash::{PasswordHash, PasswordHasher, SaltString},
        };
        use tokio::fs;

        // read the encrypted vault file
        let encrypted_data = fs::read(&vault.path).await?;
        let encrypted_vault: EncryptedVault = de::from_bytes(&encrypted_data)
            .map_err(|e| anywho!("Failed to deserialize encrypted vault: {}", e))?;

        // parse the salt
        let salt = SaltString::from_b64(&encrypted_vault.salt)?;

        // create a hash
        let password_bytes = password.as_bytes();
        let password_hash = Scrypt.hash_password(password_bytes, &salt)?.to_string();

        // parse the hash to extract the key bytes
        let parsed_hash = PasswordHash::new(&password_hash)?;
        let hash_bytes = parsed_hash
            .hash
            .ok_or_else(|| anywho!("Failed to get hash bytes"))?;

        // use the first 32 bytes of the hash as the AES-256 key
        let key_bytes = hash_bytes.as_bytes();
        if key_bytes.len() < 32 {
            return Err(anywho!("Derived key too short"));
        }

        // create cipher
        let cipher = Aes256Gcm::new_from_slice(&key_bytes[0..32])
            .map_err(|_| anywho!("Failed to create cipher"))?;

        // decrypt the data
        let nonce = Nonce::from_slice(&encrypted_vault.nonce);
        let decrypted_data = cipher
            .decrypt(
                nonce,
                Payload {
                    msg: &encrypted_vault.encrypted_data,
                    aad: b"",
                },
            )
            .map_err(|_| anywho!("Failed to decrypt: incorrect password or corrupted data"))?;

        let vault_data: VaultData = de::from_bytes(&decrypted_data)
            .map_err(|e| anywho!("Failed to deserialize vault data: {}", e))?;

        vault.state = State::Unlocked {
            data: vault_data,
            encryption_key: key_bytes[0..32].to_vec(),
        };

        Ok(vault)
    }

    /// Attempts to create an encrypted [`Vault`] given a password
    pub async fn create(password: String) -> Result<Self, anywho::Error> {
        use aes_gcm::{
            Aes256Gcm, Nonce,
            aead::{Aead, KeyInit, OsRng, Payload},
        };
        use dirs;
        use scrypt::{
            Scrypt,
            password_hash::{PasswordHash, PasswordHasher, SaltString},
        };
        use tokio::fs;

        // generate a salt for password hashing
        let salt = SaltString::generate(&mut OsRng);
        let password_bytes = password.as_bytes();

        // hash password
        let password_hash = Scrypt.hash_password(password_bytes, &salt)?.to_string();

        // parse the hash to extract the key bytes
        let parsed_hash = PasswordHash::new(&password_hash)?;
        let hash_bytes = parsed_hash
            .hash
            .ok_or_else(|| anywho!("Failed to get hash bytes"))?;

        // use the first 32 bytes of the hash as the AES-256 key
        let key_bytes = hash_bytes.as_bytes();
        if key_bytes.len() < 32 {
            return Err(anywho!("Derived key too short"));
        }

        // create empty vault data
        let vault_data = VaultData {
            entries: HashMap::new(),
        };

        let serialized_data = ser::to_string(&vault_data)?.into_bytes();

        // create cipher and encrypt
        let cipher = Aes256Gcm::new_from_slice(&key_bytes[0..32])
            .map_err(|_| anywho!("Failed to create cipher"))?;

        // generate a random nonce
        let mut nonce_bytes = [0u8; 12]; // 12 bytes for AES-GCM
        OsRng.try_fill_bytes(&mut nonce_bytes)?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        // encrypt the vault data
        let encrypted_data = cipher
            .encrypt(
                nonce,
                Payload {
                    msg: &serialized_data,
                    aad: b"",
                },
            )
            .map_err(|_| anywho!("Encryption failed"))?;

        // create the encrypted vault file structure
        let encrypted_vault = EncryptedVault {
            salt: salt.to_string(),
            nonce: nonce_bytes.to_vec(),
            encrypted_data,
        };

        // serialize the encrypted vault using RON
        let serialized_encrypted_vault = ser::to_string(&encrypted_vault)?.into_bytes();

        // ensure the directory exists
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anywho!("Could not determine data directory"))?
            .join(APP_ID);
        fs::create_dir_all(&data_dir).await?;

        // path for the vault file
        let vault_path = data_dir.join("vault");

        // save the encrypted vault file
        fs::write(&vault_path, serialized_encrypted_vault).await?;

        Ok(Self {
            path: Box::from(vault_path),
            state: State::Locked,
        })
    }

    /// Attempts to save the current [`Vault`] state
    pub async fn save(&self) -> Result<(), anywho::Error> {
        use aes_gcm::{
            Aes256Gcm, Nonce,
            aead::{Aead, KeyInit, OsRng, Payload},
        };
        use tokio::fs;

        // we can only save if the vault is unlocked
        let (vault_data, encryption_key) = match &self.state {
            State::Unlocked {
                data,
                encryption_key,
            } => (data, encryption_key),
            State::Locked => return Err(anywho!("Cannot save locked vault")),
        };

        // Serialize the vault data
        let serialized_data = ser::to_string(&vault_data)?.into_bytes();

        // generate random nonce
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // create cipher
        let cipher = Aes256Gcm::new_from_slice(encryption_key)
            .map_err(|_| anywho!("Failed to create cipher"))?;

        // encrypt the vault data
        let encrypted_data = cipher
            .encrypt(
                nonce,
                Payload {
                    msg: &serialized_data,
                    aad: b"",
                },
            )
            .map_err(|_| anywho!("Encryption failed"))?;

        // get the current salt from the file (or generate a new one if file doesn't exist)
        let salt = if self.path.exists() {
            let encrypted_data = fs::read(&self.path).await?;
            let encrypted_vault: EncryptedVault = de::from_bytes(&encrypted_data)
                .map_err(|e| anywho!("Failed to deserialize encrypted vault: {}", e))?;
            encrypted_vault.salt
        } else {
            use scrypt::password_hash::SaltString;
            SaltString::generate(&mut OsRng).to_string()
        };

        let encrypted_vault = EncryptedVault {
            salt,
            nonce: nonce_bytes.to_vec(),
            encrypted_data,
        };

        // serialize and save
        let serialized_encrypted_vault = ser::to_string(&encrypted_vault)?.into_bytes();
        fs::write(&self.path, serialized_encrypted_vault).await?;

        Ok(())
    }

    /// Get a reference to the [`Vault`]  entries if unlocked
    pub fn entries(&self) -> Option<&HashMap<super::entry::Id, Entry>> {
        match &self.state {
            State::Unlocked { data, .. } => Some(&data.entries),
            State::Locked => None,
        }
    }

    /// Tries to add or update an entry to the [`Vault`]
    pub fn upsert_entry(
        &mut self,
        mut entry: Entry,
        refresh_rate: u64,
    ) -> Result<(), anywho::Error> {
        // Check if the vault is unlocked
        let data = match &mut self.state {
            State::Unlocked { data, .. } => data,
            State::Locked => return Err(anywho!("Cannot add entry to locked vault")),
        };

        entry.generate_totp(refresh_rate)?;

        if let Some(id) = entry.id {
            if let Some(existing) = data.entries.get_mut(&id) {
                *existing = entry;
            }
        } else {
            // Insert the entry into the entries map
            let e_id = super::entry::Id(Uuid::new_v4());
            entry.id = Some(e_id);
            data.entries.insert(e_id, entry);
        }

        Ok(())
    }

    /// Substitute the entries of a [`Vault`] for antoher
    pub fn substitute_entries(
        &mut self,
        entries: HashMap<super::entry::Id, Entry>,
    ) -> Result<(), anywho::Error> {
        // Check if the vault is unlocked
        let data = match &mut self.state {
            State::Unlocked { data, .. } => data,
            State::Locked => return Err(anywho!("Cannot add entry to locked vault")),
        };

        // Insert the entry into the entries map
        data.entries = entries;

        Ok(())
    }

    /// Attempts to update the TOTP of all entries in the [`Vault`], returns the updated entries
    pub async fn update_all_totp(
        &mut self,
        refresh_rate: u64,
    ) -> Result<HashMap<super::entry::Id, Entry>, anywho::Error> {
        use tokio::task;

        // Get entries and handle the Option before spawning the task
        let entries = self
            .entries()
            .cloned()
            .ok_or_else(|| anywho!("Error getting entries"))?;

        if entries.is_empty() {
            return Err(anywho!("No entries to update"));
        }

        task::spawn_blocking(move || {
            let mut updated_entries = entries;

            for (_, entry) in updated_entries.iter_mut() {
                entry.generate_totp(refresh_rate)?
            }

            Ok(updated_entries)
        })
        .await?
    }
}
