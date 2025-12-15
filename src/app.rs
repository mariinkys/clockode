// SPDX-License-Identifier: GPL-3.0-only

use std::sync::{Arc, Mutex};

use iced::{
    Element, Length, Subscription, Task, Theme,
    time::Instant,
    widget::{container, text},
};

use crate::{
    APP_ID,
    app::{
        core::check_database,
        screen::{HomePage, Screen, UnlockDatabase, create, homepage, unlock},
        widgets::Toast,
    },
    config::Config,
};

mod core;
mod screen;
mod utils;
mod widgets;

pub struct Clockode {
    toasts: Vec<Toast>,
    config: Arc<Mutex<Config>>,
    now: Instant,
    screen: Screen,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Callback after loading the application [`Config`]
    ConfigLoaded(Result<Config, anywho::Error>),
    /// Add a new [`Toast`] to show in the app
    AddToast(Toast),
    /// Close the given [`Toast`]
    CloseToast(usize),
    /// Create Database [`Screen`] Messages
    CreateDatabase(create::Message),
    /// Unlock Database [`Screen`] Messages
    UnlockDatabase(unlock::Message),
    /// Homepage [`Screen`] Messages
    HomePage(homepage::Message),
}

impl Clockode {
    pub fn new() -> (Self, Task<Message>) {
        let (screen, task) = Screen::from_database_check(check_database());
        (
            Self {
                toasts: Vec::new(),
                config: Arc::from(Mutex::new(Config::default())),
                now: Instant::now(),
                screen,
            },
            Task::perform(Config::load(APP_ID), Message::ConfigLoaded).chain(task),
        )
    }

    pub fn update(&mut self, message: Message, now: Instant) -> Task<Message> {
        self.now = now;

        match message {
            Message::ConfigLoaded(res) => {
                match res {
                    Ok(config) => {
                        self.config = Arc::new(Mutex::from(config));
                    }
                    Err(err) => {
                        eprintln!("Error loading config: {err}");
                    }
                }
                Task::none()
            }
            Message::AddToast(toast) => {
                self.toasts.push(toast);
                Task::none()
            }
            Message::CloseToast(index) => {
                self.toasts.remove(index);
                Task::none()
            }

            Message::CreateDatabase(message) => {
                let Screen::CreateDatabase(create_database) = &mut self.screen else {
                    return Task::none();
                };

                match create_database.update(message, self.now) {
                    create::Action::None => Task::none(),
                    create::Action::Run(task) => task.map(Message::CreateDatabase),
                    create::Action::AddToast(toast) => self.update(Message::AddToast(toast), now),
                    create::Action::OpenUnlockDatabase(db_path) => {
                        let (unlock_database, task) = UnlockDatabase::new(db_path);

                        self.screen = Screen::UnlockDatabase(unlock_database);
                        task.map(Message::UnlockDatabase)
                    }
                }
            }

            Message::UnlockDatabase(message) => {
                let Screen::UnlockDatabase(unlock_database) = &mut self.screen else {
                    return Task::none();
                };

                match unlock_database.update(message, self.now) {
                    unlock::Action::None => Task::none(),
                    unlock::Action::Run(task) => task.map(Message::UnlockDatabase),
                    unlock::Action::AddToast(toast) => self.update(Message::AddToast(toast), now),
                    unlock::Action::OpenHomePage(database) => {
                        let (homepage, task) =
                            HomePage::new(Arc::new(*database), Arc::clone(&self.config));

                        self.screen = Screen::HomePage(homepage);
                        task.map(Message::HomePage)
                    }
                }
            }

            Message::HomePage(message) => {
                let Screen::HomePage(homepage) = &mut self.screen else {
                    return Task::none();
                };

                match homepage.update(message, self.now) {
                    homepage::Action::None => Task::none(),
                    homepage::Action::Run(task) => task.map(Message::HomePage),
                    homepage::Action::AddToast(toast) => self.update(Message::AddToast(toast), now),
                    homepage::Action::RunAndToast(task, toast) => Task::batch([
                        task.map(Message::HomePage),
                        self.update(Message::AddToast(toast), now),
                    ]),
                }
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content = match &self.screen {
            Screen::Error(error) => container(text(error)).center(Length::Fill).into(),
            Screen::CreateDatabase(create_database) => {
                create_database.view(self.now).map(Message::CreateDatabase)
            }
            Screen::UnlockDatabase(unlock_database) => {
                unlock_database.view(self.now).map(Message::UnlockDatabase)
            }
            Screen::HomePage(homepage) => homepage.view(self.now).map(Message::HomePage),
        };

        widgets::toast::Manager::new(content, &self.toasts, Message::CloseToast).into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        match &self.screen {
            Screen::Error(_) => Subscription::none(),
            Screen::CreateDatabase(create_database) => create_database
                .subscription(self.now)
                .map(Message::CreateDatabase),
            Screen::UnlockDatabase(unlock_database) => unlock_database
                .subscription(self.now)
                .map(Message::UnlockDatabase),
            Screen::HomePage(homepage) => homepage.subscription(self.now).map(Message::HomePage),
        }
    }

    pub fn theme(&self) -> Theme {
        self.config.lock().map_or_else(
            |_| iced::Theme::Light, // fallback theme if lock fails
            |cfg| cfg.theme.clone().into(),
        )
    }
}
