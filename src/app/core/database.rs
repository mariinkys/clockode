// SPDX-License-Identifier: GPL-3.0-only

use anywho::anywho;
use keepass::{
    Database, DatabaseKey,
    db::{Entry, Group},
};
use secrecy::{ExposeSecret, SecretString};
use std::{path::PathBuf, sync::Arc, sync::Mutex};

use crate::{APP_ID, app::core::entry::ClockodeEntry};

/// Checks whether the application database already exists.
///
/// This function looks for the database file in the platform-specific
/// application data directory under [`APP_ID`].
///
/// # Returns
///
/// - `Ok(Some(path))` if the database file exists, where `path` is the full
///   path to the database file.
/// - `Ok(None)` if the database file does not exist.
/// - `Err(_)` if the system data directory cannot be determined.
///
/// # Errors
///
/// Returns an error if the platform-specific data directory is unavailable.
pub fn check_database() -> Result<Option<PathBuf>, anywho::Error> {
    let path = dirs::data_dir()
        .ok_or_else(|| anywho!("Could not determine data directory"))?
        .join(APP_ID)
        .join("database.kdbx");

    if path.exists() {
        Ok(Some(path))
    } else {
        Ok(None)
    }
}

pub async fn create_database(password: SecretString) -> Result<PathBuf, anywho::Error> {
    let path = dirs::data_dir()
        .ok_or_else(|| anywho!("Could not determine data directory"))?
        .join(APP_ID)
        .join("database.kdbx");

    smol::unblock(move || {
        // Create the directory if it does not exist
        let dir_path = path
            .parent()
            .ok_or_else(|| anywho!("Database path has no parent directory"))?;
        std::fs::create_dir_all(dir_path)?;

        let mut db = Database::new(Default::default());
        db.meta.database_name = Some(String::from("Clockode Database"));
        let group = Group::new("Default Group");

        db.root.add_child(group);

        db.save(
            &mut std::fs::File::create(&path)?,
            DatabaseKey::new().with_password(password.expose_secret()),
        )?;

        Ok(path)
    })
    .await
}

pub async fn unlock_database(
    path: PathBuf,
    password: SecretString,
) -> Result<ClockodeDatabase, anywho::Error> {
    smol::unblock(move || {
        let mut file = std::fs::File::open(&path)?;
        let key = DatabaseKey::new().with_password(password.expose_secret());
        let _db = Database::open(&mut file, key).map_err(|e| match e {
            keepass::error::DatabaseOpenError::Key(_) => anywho!("Incorrect Password"),
            other => other.into(),
        })?;

        Ok(ClockodeDatabase {
            path: Box::from(path),
            password: Box::from(password),
            lock: Arc::new(Mutex::new(())),
        })
    })
    .await
}

#[derive(Debug, Clone)]
pub struct ClockodeDatabase {
    path: Box<PathBuf>,
    password: Box<SecretString>,
    lock: Arc<std::sync::Mutex<()>>, // We use this to prevent Race Condition / Data Loss
}

impl ClockodeDatabase {
    pub async fn list_entries(&self) -> Result<Vec<ClockodeEntry>, anywho::Error> {
        let lock = self.lock.clone();

        let path = self.path.clone();
        let password = self.password.clone();

        smol::unblock(move || {
            let _guard = lock
                .lock()
                .map_err(|e| anywho!("Database lock poisoned: {}", e))?;

            let mut file = std::fs::File::open(&*path)?;
            let key = DatabaseKey::new().with_password(password.expose_secret());
            let db = Database::open(&mut file, key)?;
            drop(file); // this should't be needed here because we only read, I just added it for consistency

            let entries = db
                .root
                .children
                .iter()
                .find_map(|node| {
                    if let keepass::db::Node::Group(g) = node
                        && g.name == "Default Group"
                    {
                        return Some(g.entries());
                    }
                    None
                })
                .map(|entries_iter| {
                    let mut v = entries_iter
                        .into_iter()
                        .map(|e| ClockodeEntry::try_from(e.to_owned()))
                        .collect::<Result<Vec<_>, _>>()?;

                    v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                    Ok::<Vec<ClockodeEntry>, anywho::Error>(v)
                })
                .transpose()?
                .unwrap_or_else(Vec::new);

            Ok(entries)
        })
        .await
    }

    pub async fn add_entry(&self, entry: ClockodeEntry) -> Result<(), anywho::Error> {
        let lock = self.lock.clone();

        let path = self.path.clone();
        let password = self.password.clone();

        smol::unblock(move || {
            let _guard = lock
                .lock()
                .map_err(|e| anywho!("Database lock poisoned: {}", e))?;

            let mut file = std::fs::File::open(&*path)?;
            let key = DatabaseKey::new().with_password(password.expose_secret());
            let mut db = Database::open(&mut file, key)?;
            drop(file);

            let keepass_entry = Entry::from(entry);

            let target_group = db
                .root
                .children
                .iter_mut()
                .find_map(|node| {
                    if let keepass::db::Node::Group(g) = node
                        && g.name == "Default Group"
                    {
                        return Some(g);
                    }
                    None
                })
                .ok_or_else(|| anywho!("Default Group not found"))?;

            target_group.add_child(keepass_entry);

            db.save(
                &mut std::fs::File::create(&*path)?,
                DatabaseKey::new().with_password(password.expose_secret()),
            )?;

            Ok(())
        })
        .await
    }

    pub async fn update_entry(&self, entry: ClockodeEntry) -> Result<(), anywho::Error> {
        let lock = self.lock.clone();

        let path = self.path.clone();
        let password = self.password.clone();

        smol::unblock(move || {
            let _guard = lock
                .lock()
                .map_err(|e| anywho!("Database lock poisoned: {}", e))?;

            let mut file = std::fs::File::open(&*path)?;
            let key = DatabaseKey::new().with_password(password.expose_secret());
            let mut db = Database::open(&mut file, key)?;
            drop(file);

            let entry_id = entry
                .id
                .ok_or_else(|| anywho!("Cannot update entry without UUID"))?;

            let target_group = db
                .root
                .children
                .iter_mut()
                .find_map(|node| {
                    if let keepass::db::Node::Group(g) = node
                        && g.name == "Default Group"
                    {
                        return Some(g);
                    }
                    None
                })
                .ok_or_else(|| anywho!("Default Group not found"))?;

            // Find and update the entry
            let entry_found = target_group
                .children
                .iter_mut()
                .find_map(|node| {
                    if let keepass::db::Node::Entry(e) = node
                        && e.get_uuid() == &entry_id
                    {
                        return Some(e);
                    }
                    None
                })
                .ok_or_else(|| anywho!("Entry with UUID {} not found", entry_id))?;

            let updated_keepass_entry = Entry::from(entry);

            // Update all fields from the new entry
            entry_found.fields = updated_keepass_entry.fields;

            db.save(
                &mut std::fs::File::create(&*path)?,
                DatabaseKey::new().with_password(password.expose_secret()),
            )?;

            Ok(())
        })
        .await
    }

    pub async fn delete_entry(&self, entry_id: uuid::Uuid) -> Result<(), anywho::Error> {
        let lock = self.lock.clone();

        let path = self.path.clone();
        let password = self.password.clone();

        smol::unblock(move || {
            let _guard = lock
                .lock()
                .map_err(|e| anywho!("Database lock poisoned: {}", e))?;

            let mut file = std::fs::File::open(&*path)?;
            let key = DatabaseKey::new().with_password(password.expose_secret());
            let mut db = Database::open(&mut file, key)?;
            drop(file);

            let target_group = db
                .root
                .children
                .iter_mut()
                .find_map(|node| {
                    if let keepass::db::Node::Group(g) = node
                        && g.name == "Default Group"
                    {
                        return Some(g);
                    }
                    None
                })
                .ok_or_else(|| anywho!("Default Group not found"))?;

            let entry_index = target_group
                .children
                .iter()
                .position(|node| {
                    if let keepass::db::Node::Entry(e) = node {
                        return e.get_uuid() == &entry_id;
                    }
                    false
                })
                .ok_or_else(|| anywho!("Entry with UUID {} not found", entry_id))?;

            target_group.children.remove(entry_index);

            db.save(
                &mut std::fs::File::create(&*path)?,
                DatabaseKey::new().with_password(password.expose_secret()),
            )?;

            Ok(())
        })
        .await
    }

    // Import the content given in standard totp
    pub async fn import_content(&self, file_path: PathBuf) -> Result<(), anywho::Error> {
        // Read the import file
        let content = std::fs::read_to_string(&file_path)
            .map_err(|e| anywho!("Failed to read import file: {}", e))?;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // we use from_url unchecked because of the same reason we can't use TOTP::new
            // Don't use TOTP::new() because it enforces validation and some secrets (ej: microsoft)
            // that are xxxx xxxx xxxx xxxx will fail here if we use ::new() with error:
            // Failed to construct TOTP object: The length of the shared secret MUST be at least 128 bits. 80 bits is not enough
            match totp_rs::TOTP::from_url_unchecked(line) {
                Ok(totp) => {
                    let name = if totp.account_name.trim().is_empty() {
                        "Default".to_string()
                    } else {
                        totp.account_name.clone()
                    };

                    let entry = ClockodeEntry {
                        id: None,
                        name,
                        totp,
                    };

                    self.add_entry(entry).await?;
                }
                Err(e) => {
                    eprintln!("Warning: Failed to parse TOTP URL '{}': {}", line, e);
                }
            }
        }

        Ok(())
    }

    // Export content to standard
    pub async fn export_content(&self, file_path: PathBuf) -> Result<(), anywho::Error> {
        let entries = self.list_entries().await?;

        if entries.is_empty() {
            return Err(anywho!("No entries found to export"));
        }

        let mut export_content = String::new();

        for entry in entries {
            let url = entry.totp.get_url();
            export_content.push_str(&url);
            export_content.push('\n');
        }

        std::fs::write(&file_path, export_content)
            .map_err(|e| anywho!("Failed to write export file: {}", e))?;

        Ok(())
    }
}
