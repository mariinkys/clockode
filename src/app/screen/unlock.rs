// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use iced::{
    Alignment,
    Length::{self},
    Subscription, Task, event,
    keyboard::{self, Key, Modifiers, key::Named},
    time::Instant,
    widget::{
        button, column, container,
        operation::{focus_next, focus_previous},
        text, text_input,
    },
};
use keepass::Database;

use crate::app::core::unlock_database;

pub struct UnlockDatabase {
    db_path: PathBuf,
    inputs: PageInputs,
}

#[derive(Debug, Clone)]
pub enum Message {
    Hotkey(Hotkey),

    UpdatePassword(String),
    Submit,

    DatabaseUnlocked(Box<Result<Database, anywho::Error>>),
}

pub enum Action {
    /// Does nothing
    None,
    /// Ask parent to run an [`iced::Task`]
    Run(Task<Message>),
    /// Ask parent to open the [`Screen::HomePage`]
    OpenHomePage(Box<Database>),
}

impl UnlockDatabase {
    pub fn new(db_path: PathBuf) -> (Self, Task<Message>) {
        (
            Self {
                db_path,
                inputs: PageInputs::default(),
            },
            Task::none(),
        )
    }

    pub fn view(&self, _now: Instant) -> iced::Element<'_, Message> {
        let content = column![
            container(text("Unlock Database").width(Length::Shrink))
                .width(Length::Fill)
                .align_x(Alignment::Center),
            text_input("Password", &self.inputs.password)
                .secure(true)
                .on_input(Message::UpdatePassword)
                .on_submit_maybe(self.inputs.valid().then_some(Message::Submit)),
            button("Unlock")
                .on_press_maybe(self.inputs.valid().then_some(Message::Submit))
                .width(Length::Fill)
        ]
        .spacing(3.);

        container(content).center(Length::Fill).into()
    }

    pub fn update(&mut self, message: Message, _now: Instant) -> Action {
        match message {
            Message::Hotkey(hotkey) => match hotkey {
                Hotkey::Tab(modifiers) => {
                    if modifiers.shift() {
                        Action::Run(focus_previous())
                    } else {
                        Action::Run(focus_next())
                    }
                }
            },

            Message::UpdatePassword(v) => {
                self.inputs.password = v;
                Action::None
            }
            Message::Submit => Action::Run(Task::perform(
                unlock_database(self.db_path.clone(), self.inputs.password.clone()),
                |res| Message::DatabaseUnlocked(Box::from(res)),
            )),
            Message::DatabaseUnlocked(res) => match *res {
                Ok(db) => Action::OpenHomePage(Box::from(db)),
                Err(err) => todo!(),
            },
        }
    }

    pub fn subscription(&self, _now: Instant) -> Subscription<Message> {
        event::listen_with(handle_event)
    }
}

/// Holds the state for the different inputs of the page
#[derive(Default)]
struct PageInputs {
    password: String,
}

impl PageInputs {
    /// Returns true if the inputs are ready for submission
    fn valid(&self) -> bool {
        !self.password.is_empty()
    }
}

//
// SUBSCRIPTIONS
//

#[derive(Debug, Clone)]
pub enum Hotkey {
    Tab(Modifiers),
}

fn handle_event(event: event::Event, _: event::Status, _: iced::window::Id) -> Option<Message> {
    #[allow(clippy::collapsible_match)]
    match event {
        event::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => match key {
            Key::Named(Named::Tab) => Some(Message::Hotkey(Hotkey::Tab(modifiers))),
            _ => None,
        },
        _ => None,
    }
}
