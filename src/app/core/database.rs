// SPDX-License-Identifier: GPL-3.0-only

use anywho::anywho;
use keepass::{Database, DatabaseKey, db::Group};
use std::path::PathBuf;

use crate::APP_ID;

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

pub async fn create_database(password: String) -> Result<PathBuf, anywho::Error> {
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
            DatabaseKey::new().with_password(&password),
        )?;

        Ok(path)
    })
    .await
}

pub async fn unlock_database(path: PathBuf, password: String) -> Result<Database, anywho::Error> {
    smol::unblock(move || {
        let mut file = std::fs::File::open(path)?;
        let key = DatabaseKey::new().with_password(&password);
        let db = Database::open(&mut file, key)?;

        Ok(db)
    })
    .await
}
