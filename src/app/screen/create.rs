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
        space, svg, text, text_input,
    },
};

use crate::{
    APP_ICON,
    app::{core::create_database, utils::style, widgets::Toast},
};

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
        let password_form = container(
            column![
                column![
                    text("Password")
                        .size(style::font_size::BODY)
                        .style(style::label_text),
                    text_input("Enter a strong password", &self.inputs.password)
                        .secure(true)
                        .on_input(Message::UpdatePassword)
                        .on_submit_maybe(self.inputs.valid().then_some(Message::Submit))
                        .padding(12)
                        .size(style::font_size::MEDIUM)
                ]
                .spacing(style::spacing::TINY),
                column![
                    text("Confirm Password")
                        .size(style::font_size::BODY)
                        .style(style::label_text),
                    text_input("Re-enter your password", &self.inputs.repeat_password)
                        .secure(true)
                        .on_input(Message::UpdateRepeatPassword)
                        .on_submit_maybe(self.inputs.valid().then_some(Message::Submit))
                        .padding(12)
                        .size(style::font_size::MEDIUM)
                ]
                .spacing(style::spacing::TINY),
                // Password strength hint
                text("Choose a strong password. You'll need it to access your codes.")
                    .size(style::font_size::SMALL)
                    .width(Length::Fill)
                    .align_x(Alignment::Center)
                    .style(style::muted_text),
                // Create button
                button(
                    text("Create Database")
                        .size(style::font_size::MEDIUM)
                        .width(Length::Fill)
                        .align_x(Alignment::Center)
                )
                .on_press_maybe(self.inputs.valid().then_some(Message::Submit))
                .padding(16)
                .width(Length::Fill)
                .style(style::primary_submit_button),
            ]
            .spacing(style::spacing::XLARGE),
        )
        .max_width(500)
        .padding(32)
        .style(style::card_container);

        let content = column![
            // App icon
            container(svg(svg::Handle::from_memory(APP_ICON)).width(80).height(80))
                .width(Length::Fill)
                .align_x(Alignment::Center),
            space().height(Length::Fixed(20.)),
            // Welcome text
            column![
                text("Welcome to Clockode")
                    .align_x(Alignment::Center)
                    .size(style::font_size::HERO),
                text("Create a password to secure your TOTP entries")
                    .align_x(Alignment::Center)
                    .size(style::font_size::BODY)
                    .style(style::subtitle_text),
            ]
            .spacing(style::spacing::SMALL)
            .align_x(Alignment::Center),
            space().height(Length::Fixed(32.)),
            password_form,
        ]
        .spacing(0)
        .align_x(Alignment::Center);

        container(content).center(Length::Fill).padding(20).into()
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
