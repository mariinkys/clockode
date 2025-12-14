// SPDX-License-Identifier: GPL-3.0-only

use iced::{
    Element,
    Length::{self},
    Subscription, Task,
    time::Instant,
    widget::{button, column, container, pick_list, row, text, text_input},
};
use totp_rs::Algorithm;

use crate::app::{
    core::ClockodeEntry,
    utils::{ALL_ALGORITHMS, InputableClockodeEntry},
    widgets::Toast,
};

pub struct UpsertPage {
    entry: InputableClockodeEntry,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Go back a screen
    Back,

    /// Input update of the various available fields
    InputUpdated(TOTPEntryInput),
    /// Submit the changes
    Submit,
}

pub enum Action {
    /// Does nothing
    None,
    /// Go back a screen
    Back,
    // Ask parent to run an [`iced::Task`]
    // Run(Task<Message>),
    /// Add a new [`Toast`] to show
    AddToast(Toast),
    /// Ask the parent to update the given [`ClockodeEntry`]
    UpdateEntry(ClockodeEntry),
    /// Ask the parent to create the given [`ClockodeEntry`]
    CreateEntry(ClockodeEntry),
}

/// Represents the different inputs the user can perfrom on the upsert screen
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone)]
pub enum TOTPEntryInput {
    UpdateName(String),
    UpdateAlgorithm(Algorithm),
    UpdateDigits(String),
    UpdateStep(String),
    UpdateSecret(String),
    UpdateIssuer(String),
    UpdateAccountName(String),
}

impl UpsertPage {
    pub fn new(entry: Option<ClockodeEntry>) -> (Self, Task<Message>) {
        let entry = entry.map(InputableClockodeEntry::from).unwrap_or_default();

        (Self { entry }, Task::none())
    }

    pub fn view(&self, _now: Instant) -> iced::Element<'_, Message> {
        let header = header_view();
        let content = upsert_entry_view(&self.entry);

        container(
            container(column![header, content])
                .padding(5.)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .center(Length::Fill)
        .into()
    }

    pub fn update(&mut self, message: Message, _now: Instant) -> Action {
        match message {
            Message::Back => Action::Back,

            Message::InputUpdated(input) => {
                match input {
                    TOTPEntryInput::UpdateName(v) => self.entry.name = v,
                    TOTPEntryInput::UpdateAlgorithm(v) => self.entry.algorithm = v,
                    TOTPEntryInput::UpdateDigits(v) => {
                        if !v.is_empty() {
                            if let Ok(parsed) = v.parse::<usize>() {
                                self.entry.digits = parsed;
                            }
                        } else {
                            self.entry.digits = 0;
                        }
                    }
                    TOTPEntryInput::UpdateStep(v) => {
                        if !v.is_empty() {
                            if let Ok(parsed) = v.parse::<u64>() {
                                self.entry.step = parsed;
                            }
                        } else {
                            self.entry.step = 0;
                        }
                    }
                    TOTPEntryInput::UpdateSecret(v) => self.entry.secret = v,
                    TOTPEntryInput::UpdateIssuer(v) => {
                        if v.is_empty() {
                            self.entry.issuer = None;
                        } else {
                            self.entry.issuer = Some(v);
                        }
                    }
                    TOTPEntryInput::UpdateAccountName(v) => self.entry.account_name = v,
                }
                Action::None
            }
            Message::Submit => {
                if self.entry.valid() {
                    let clockode_entry_res = ClockodeEntry::try_from(self.entry.clone());
                    match clockode_entry_res {
                        Ok(clockode_entry) => {
                            if clockode_entry.id.is_some() {
                                Action::UpdateEntry(clockode_entry)
                            } else {
                                Action::CreateEntry(clockode_entry)
                            }
                        }
                        Err(err) => Action::AddToast(Toast::error_toast(err)),
                    }
                } else {
                    Action::AddToast(Toast::error_toast("Invalid TOTP Entity"))
                }
            }
        }
    }

    pub fn subscription(&self, _now: Instant) -> Subscription<Message> {
        Subscription::none()
    }
}

/// View of the header of this screen
fn header_view<'a>() -> Element<'a, Message> {
    row![button("Back").on_press(Message::Back)]
        .width(Length::Fill)
        .height(Length::Fixed(30.))
        .into()
}

fn upsert_entry_view<'a>(entry: &'a InputableClockodeEntry) -> Element<'a, Message> {
    let button_content = if entry.uuid.is_some() {
        String::from("Update")
    } else {
        String::from("Create")
    };

    column![
        text_input("Name", &entry.name)
            .on_input(|v| Message::InputUpdated(TOTPEntryInput::UpdateName(v))),
        pick_list(ALL_ALGORITHMS, Some(&entry.algorithm), |v| {
            Message::InputUpdated(TOTPEntryInput::UpdateAlgorithm(v))
        }),
        text_input("Digits", &entry.digits.to_string())
            .on_input(|v| Message::InputUpdated(TOTPEntryInput::UpdateDigits(v))),
        text_input("Step", &entry.step.to_string())
            .on_input(|v| Message::InputUpdated(TOTPEntryInput::UpdateStep(v))),
        text_input("Secret", &entry.secret)
            .on_input(|v| Message::InputUpdated(TOTPEntryInput::UpdateSecret(v))),
        text_input("Issuer", entry.issuer.as_deref().unwrap_or(""))
            .on_input(|v| Message::InputUpdated(TOTPEntryInput::UpdateIssuer(v))),
        text_input("Account Name", &entry.account_name.to_string())
            .on_input(|v| Message::InputUpdated(TOTPEntryInput::UpdateAccountName(v))),
        button(text(button_content)).on_press(Message::Submit)
    ]
    .spacing(3.)
    .into()
}
