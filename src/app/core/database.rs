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
        let _db = Database::open(&mut file, key)?;

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
                    entries_iter
                        .into_iter()
                        .map(|e| ClockodeEntry::try_from(e.to_owned()))
                        .collect::<Result<Vec<ClockodeEntry>, _>>()
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
}
