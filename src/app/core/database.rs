// SPDX-License-Identifier: GPL-3.0-only

use anywho::anywho;
use keepass::{Database, DatabaseKey, config::DatabaseVersion};
use secrecy::{ExposeSecret, SecretString};
use std::{
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::SystemTime,
};
use tracing::{info, warn};

use crate::{
    APP_ID,
    app::core::entry::{ClockodeEntry, update_clockode_entry_in_keepass},
};

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

    info!("DATABASE_PATH {:?}", &path);

    if path.exists() {
        Ok(Some(path))
    } else {
        Ok(None)
    }
}

/// Reads the current modification time of a file, if available.
fn read_mtime(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Saves the database atomically and returns the resulting file mtime
fn save_database_atomic(
    db: &mut Database,
    path: &std::path::Path,
    password: &SecretString,
) -> Result<Option<SystemTime>, anywho::Error> {
    db.config.version = DatabaseVersion::KDB4(1);

    // serialize entirely into memory first. If this fails, the file on disk is untouched.
    let mut buf: Vec<u8> = Vec::new();
    db.save(
        &mut buf,
        DatabaseKey::new().with_password(password.expose_secret()),
    )?;

    // write to a temporary file in the same directory.
    let dir = path
        .parent()
        .ok_or_else(|| anywho!("Database path has no parent directory"))?;
    let tmp_path = dir.join("database.kdbx.tmp");

    let mtime = {
        let mut f = std::fs::File::create(&tmp_path)?;
        f.write_all(&buf)?;
        // make sure the bytes actually hit the disk before we swap files,
        f.sync_all()?;

        // capture the mtime the destination file will have after the rename, tthis is what lets callers recognize (and ignore) filesystem-watcher events caused by this very save.
        f.metadata().and_then(|m| m.modified()).ok()
    };  // drop file handle

    // replace the old database with the new one
    if let Err(e) = std::fs::rename(&tmp_path, path) {
        // remove tmp file
        let _ = std::fs::remove_file(&tmp_path);
        return Err(anywho!("Failed to replace database file: {}", e));
    }

    Ok(mtime)
}

/// Stores `mtime` as the last file state this app instance knows about.
fn record_known_mtime(slot: &Mutex<Option<SystemTime>>, mtime: Option<SystemTime>) {
    if let Ok(mut guard) = slot.lock() {
        *guard = mtime;
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

        let mut db = Database::new();
        db.meta.database_name = Some(String::from("Clockode Database"));

        let mut root = db.root_mut();
        let mut group = root.add_group();
        group.name = String::from("Default Group");

        let _ = save_database_atomic(&mut db, &path, &password)?;

        Ok(path)
    })
    .await
}

pub async fn unlock_database(
    path: PathBuf,
    password: SecretString,
) -> Result<ClockodeDatabase, anywho::Error> {
    smol::unblock(move || {
        let known_mtime = read_mtime(&path);

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
            known_mtime: Arc::new(Mutex::new(known_mtime)),
        })
    })
    .await
}

#[derive(Debug, Clone)]
pub struct ClockodeDatabase {
    path: Box<PathBuf>,
    password: Box<SecretString>,
    lock: Arc<std::sync::Mutex<()>>, // We use this to prevent Race Condition / Data Loss
    /// The file mtime corresponding to the last read or write this instance performed. Used to tell our own saves apart from changes made by another process.
    known_mtime: Arc<Mutex<Option<SystemTime>>>,
}

impl ClockodeDatabase {
    /// Path of the database file on disk.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Returns `true` if the file on disk differs from the last state this
    /// instance read or wrote — i.e. the change came from *another* process.
    ///
    /// We uuse this in the filesystem-watcher handler to skip reloads triggered
    /// by our own saves.
    ///
    /// Errs on the side of `true`: if the mtime can't be read (file deleted,
    /// permissions...), a reload is triggered so the failure surfaces to the
    /// user instead of being silently swallowed.
    pub fn has_changed_on_disk(&self) -> bool {
        let current = read_mtime(&self.path);

        let known = self.known_mtime.lock().ok().and_then(|guard| *guard);

        match (current, known) {
            (Some(current), Some(known)) => current != known,
            _ => true,
        }
    }

    pub async fn list_entries(&self) -> Result<Vec<ClockodeEntry>, anywho::Error> {
        info!("Listing database entries");

        let lock = self.lock.clone();

        let path = self.path.clone();
        let password = self.password.clone();
        let known_mtime = self.known_mtime.clone();

        smol::unblock(move || {
            let _guard = lock
                .lock()
                .map_err(|e| anywho!("Database lock poisoned: {}", e))?;

            // Capture the mtime before opening
            let mtime = read_mtime(&path);

            let mut file = std::fs::File::open(&*path)?;
            let key = DatabaseKey::new().with_password(password.expose_secret());
            let db = Database::open(&mut file, key)?;
            drop(file); // this should't be needed here because we only read, I just added it for consistency

            record_known_mtime(&known_mtime, mtime);

            let entries = db
                .root()
                .group_by_name("Default Group")
                .map(|g| {
                    let mut v = g
                        .entries()
                        .map(|e| ClockodeEntry::try_from(e.to_owned()))
                        .collect::<Result<Vec<_>, _>>()?;
                    v.sort_by_key(|a| a.name.to_lowercase());
                    Ok::<Vec<ClockodeEntry>, anywho::Error>(v)
                })
                .transpose()?
                .unwrap_or_else(Vec::new);

            Ok(entries)
        })
        .await
    }

    pub async fn add_entry(&self, entry: ClockodeEntry) -> Result<(), anywho::Error> {
        info!("Adding database entry");

        let lock = self.lock.clone();

        let path = self.path.clone();
        let password = self.password.clone();
        let known_mtime = self.known_mtime.clone();

        smol::unblock(move || {
            let _guard = lock
                .lock()
                .map_err(|e| anywho!("Database lock poisoned: {}", e))?;

            let mut file = std::fs::File::open(&*path)?;
            let key = DatabaseKey::new().with_password(password.expose_secret());
            let mut db = Database::open(&mut file, key)?;
            drop(file);

            let mut root = db.root_mut();
            let mut target_group = root
                .group_by_name_mut("Default Group")
                .ok_or_else(|| anywho!("Default Group not found"))?;
            let mut keepass_entry = target_group.add_entry();

            update_clockode_entry_in_keepass(entry, &mut keepass_entry);

            let mtime = save_database_atomic(&mut db, &path, &password)?;
            record_known_mtime(&known_mtime, mtime);

            Ok(())
        })
        .await
    }

    pub async fn update_entry(&self, entry: ClockodeEntry) -> Result<(), anywho::Error> {
        info!("Updating database entry");

        let lock = self.lock.clone();

        let path = self.path.clone();
        let password = self.password.clone();
        let known_mtime = self.known_mtime.clone();

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

            let mut root = db.root_mut();
            let mut target_group = root
                .group_by_name_mut("Default Group")
                .ok_or_else(|| anywho!("Default Group not found"))?;

            // Find and update the entry
            let entry_id = target_group
                .entry_ids()
                .find(|e| e.uuid() == entry_id)
                .ok_or_else(|| anywho!("Entry with UUID {} not found", entry_id))?;
            let mut entry_found = target_group
                .entry_mut(entry_id)
                .ok_or_else(|| anywho!("Entry with UUID {} not found", entry_id))?;

            update_clockode_entry_in_keepass(entry, &mut entry_found);

            let mtime = save_database_atomic(&mut db, &path, &password)?;
            record_known_mtime(&known_mtime, mtime);

            Ok(())
        })
        .await
    }

    pub async fn delete_entry(&self, entry_id: uuid::Uuid) -> Result<(), anywho::Error> {
        info!("Deleting database entry");

        let lock = self.lock.clone();

        let path = self.path.clone();
        let password = self.password.clone();
        let known_mtime = self.known_mtime.clone();

        smol::unblock(move || {
            let _guard = lock
                .lock()
                .map_err(|e| anywho!("Database lock poisoned: {}", e))?;

            let mut file = std::fs::File::open(&*path)?;
            let key = DatabaseKey::new().with_password(password.expose_secret());
            let mut db = Database::open(&mut file, key)?;
            drop(file);

            let mut root = db.root_mut();
            let mut target_group = root
                .group_by_name_mut("Default Group")
                .ok_or_else(|| anywho!("Default Group not found"))?;

            let entry_id = target_group
                .entry_ids()
                .find(|e| e.uuid() == entry_id)
                .ok_or_else(|| anywho!("Entry with UUID {} not found", entry_id))?;
            let entry_found = target_group
                .entry_mut(entry_id)
                .ok_or_else(|| anywho!("Entry with UUID {} not found", entry_id))?;

            entry_found.remove();

            let mtime = save_database_atomic(&mut db, &path, &password)?;
            record_known_mtime(&known_mtime, mtime);

            Ok(())
        })
        .await
    }

    // Import the content given in standard totp
    pub async fn import_content(&self, file_path: PathBuf) -> Result<(), anywho::Error> {
        info!("Importing content to database");

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
                    warn!("Warning: Failed to parse TOTP URL '{}': {}", line, e);
                }
            }
        }

        Ok(())
    }

    // Export content to standard
    pub async fn export_content(&self, file_path: PathBuf) -> Result<(), anywho::Error> {
        info!("Exporting database content");

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
