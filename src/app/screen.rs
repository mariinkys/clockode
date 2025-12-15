// SPDX-License-Identifier: GPL-3.0-only

pub mod create;
pub mod homepage;
pub mod unlock;

pub use create::CreateDatabase;
pub use homepage::HomePage;
use iced::Task;
pub use unlock::UnlockDatabase;

pub enum Screen {
    Error(String),
    CreateDatabase(CreateDatabase),
    UnlockDatabase(UnlockDatabase),
    HomePage(HomePage),
}

impl Screen {
    /// Creates the initial application [`Screen`] based on the result of a
    /// database existence check.
    ///
    /// This function interprets the outcome of a database check and selects
    /// the appropriate screen to display:
    ///
    /// - If a database path is present, the [`UnlockDatabase`] screen is shown.
    /// - If no database exists, the [`CreateDatabase`] screen is shown.
    /// - If an error occurred while checking, an [`Screen::Error`] screen is shown.
    ///
    /// Along with the selected screen, this function also returns an
    /// [`iced::Task`] used to initialize the screen and dispatch the
    /// corresponding application message.
    ///
    /// # Parameters
    ///
    /// - `response`: The result of a database check, returned by `check_database()`.  
    ///
    /// # Returns
    ///
    /// A tuple containing:
    ///
    /// - The initial [`Screen`] to display.
    /// - An [`iced::Task`] that initializes the screen and maps its output
    ///   into a [`crate::app::Message`].
    ///
    pub fn from_database_check(
        response: Result<Option<std::path::PathBuf>, anywho::Error>,
    ) -> (Self, Task<crate::app::Message>) {
        match response {
            Ok(maybe_db) => match maybe_db {
                Some(db_path) => {
                    let (unlock_database, task) = UnlockDatabase::new(db_path);
                    (
                        crate::app::screen::Screen::UnlockDatabase(unlock_database),
                        task.map(crate::app::Message::UnlockDatabase),
                    )
                }
                None => {
                    let (create_database, task) = CreateDatabase::new();
                    (
                        crate::app::screen::Screen::CreateDatabase(create_database),
                        task.map(crate::app::Message::CreateDatabase),
                    )
                }
            },
            Err(err) => (Screen::Error(err.to_string()), Task::none()),
        }
    }
}
