// SPDX-License-Identifier: GPL-3.0-only

use std::path::{Path, PathBuf};

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
    app::{
        core::{ClockodeDatabase, unlock_database},
        utils::style,
        widgets::Toast,
    },
};

pub struct UnlockDatabase {
    db_path: PathBuf,
    inputs: PageInputs,
}

#[derive(Debug, Clone)]
pub enum Message {
    Hotkey(Hotkey),

    UpdatePassword(String),
    Submit,

    DatabaseUnlocked(Box<Result<ClockodeDatabase, anywho::Error>>),
}

pub enum Action {
    /// Does nothing
    None,
    /// Ask parent to run an [`iced::Task`]
    Run(Task<Message>),
    /// Ask parent to open the [`Screen::HomePage`]
    OpenHomePage(Box<ClockodeDatabase>),
    /// Add a new [`Toast`] to show
    AddToast(Toast),
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
        let unlock_form = container(
            column![
                column![
                    text("Password")
                        .size(style::font_size::BODY)
                        .style(style::label_text),
                    text_input("Enter your password", &self.inputs.password)
                        .secure(true)
                        .on_input(Message::UpdatePassword)
                        .on_submit_maybe(self.inputs.valid().then_some(Message::Submit))
                        .padding(12)
                        .size(style::font_size::MEDIUM)
                ]
                .spacing(style::spacing::TINY),
                button(
                    text("Unlock Database")
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
            container(svg(svg::Handle::from_memory(APP_ICON)).width(80).height(80))
                .width(Length::Fill)
                .align_x(Alignment::Center),
            space().height(Length::Fixed(20.)),
            column![
                text("Welcome Back")
                    .align_x(Alignment::Center)
                    .size(style::font_size::HERO),
                text("Enter your password to unlock your TOTP codes")
                    .align_x(Alignment::Center)
                    .size(style::font_size::BODY)
                    .style(style::subtitle_text),
            ]
            .spacing(style::spacing::SMALL)
            .align_x(Alignment::Center),
            space().height(Length::Fixed(32.)),
            unlock_form,
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
                Hotkey::Submit => {
                    if self.inputs.valid() {
                        submit_form(&self.inputs, &self.db_path)
                    } else {
                        Action::None
                    }
                }
            },

            Message::UpdatePassword(v) => {
                self.inputs.password = v;
                Action::None
            }
            Message::Submit => {
                if self.inputs.valid() {
                    submit_form(&self.inputs, &self.db_path)
                } else {
                    Action::None
                }
            }
            Message::DatabaseUnlocked(res) => match *res {
                Ok(db) => Action::OpenHomePage(Box::from(db)),
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
    Submit,
}

fn handle_event(event: event::Event, _: event::Status, _: iced::window::Id) -> Option<Message> {
    #[allow(clippy::collapsible_match)]
    match event {
        event::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => match key {
            Key::Named(Named::Tab) => Some(Message::Hotkey(Hotkey::Tab(modifiers))),
            Key::Named(Named::Enter) => Some(Message::Hotkey(Hotkey::Submit)),
            _ => None,
        },
        _ => None,
    }
}

// HELPERS

fn submit_form(inputs: &PageInputs, db_path: &Path) -> Action {
    Action::Run(Task::perform(
        unlock_database(db_path.to_path_buf(), inputs.password.clone().into()),
        |res| Message::DatabaseUnlocked(Box::from(res)),
    ))
}
