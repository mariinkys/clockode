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

use crate::app::{core::create_database, widgets::Toast};

pub struct CreateDatabase {
    inputs: PageInputs,
}

#[derive(Debug, Clone)]
pub enum Message {
    Hotkey(Hotkey),

    UpdatePassword(String),
    UpdateRepeatPassword(String),
    Submit,

    DatabaseCreated(Result<PathBuf, anywho::Error>),
}

pub enum Action {
    /// Does nothing
    None,
    /// Ask parent to run an [`iced::Task`]
    Run(Task<Message>),
    /// Ask parent to open the [`Screen::UnlockDatabase`]
    OpenUnlockDatabase(PathBuf),
    /// Add a new [`Toast`] to show
    AddToast(Toast),
}

impl CreateDatabase {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                inputs: PageInputs::default(),
            },
            Task::none(),
        )
    }

    pub fn view(&self, _now: Instant) -> iced::Element<'_, Message> {
        let content = column![
            container(text("Create Database").width(Length::Shrink))
                .width(Length::Fill)
                .align_x(Alignment::Center),
            text_input("Password", &self.inputs.password)
                .secure(true)
                .on_input(Message::UpdatePassword)
                .on_submit_maybe(self.inputs.valid().then_some(Message::Submit)),
            text_input("Repeat Password", &self.inputs.repeat_password)
                .secure(true)
                .on_input(Message::UpdateRepeatPassword)
                .on_submit_maybe(self.inputs.valid().then_some(Message::Submit)),
            button("Create")
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
            Message::UpdateRepeatPassword(v) => {
                self.inputs.repeat_password = v;
                Action::None
            }
            Message::Submit => Action::Run(Task::perform(
                create_database(self.inputs.password.clone().into()),
                Message::DatabaseCreated,
            )),

            Message::DatabaseCreated(result) => match result {
                Ok(db_path) => Action::OpenUnlockDatabase(db_path),
                Err(err) => Action::AddToast(Toast::error_toast(err)),
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
    repeat_password: String,
}

impl PageInputs {
    /// Returns true if the inputs are ready for submission
    fn valid(&self) -> bool {
        self.password.eq(&self.repeat_password)
            && !self.password.is_empty()
            && self.password.len() > 3
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
