// SPDX-License-Identifier: GPL-3.0-only

use iced::{
    Alignment, Element,
    Length::{self},
    Subscription, Task,
    time::Instant,
    widget::{button, column, container, pick_list, row, space, text, text_input},
};
use totp_rs::Algorithm;

use crate::{
    app::{
        core::ClockodeEntry,
        utils::{ALL_ALGORITHMS, InputableClockodeEntry, style},
        widgets::Toast,
    },
    icons,
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
    /// Delete the currently editing entry
    Delete,
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
    /// Ask the parent to delete the [`ClockodeEntry`] with the give [`uuid::Uuid`]
    DeleteEntry(uuid::Uuid),
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
        let header = header_view(&self.entry);
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
            Message::Delete => {
                if let Some(id) = self.entry.uuid {
                    Action::DeleteEntry(id)
                } else {
                    Action::None
                }
            }
        }
    }

    pub fn subscription(&self, _now: Instant) -> Subscription<Message> {
        Subscription::none()
    }
}

/// View of the header of this screen
fn header_view<'a>(entry: &'a InputableClockodeEntry) -> Element<'a, Message> {
    let title = if entry.uuid.is_some() {
        "Edit Entry"
    } else {
        "New Entry"
    };

    row![
        // Back button
        button(
            row![
                icons::get_icon("go-previous-symbolic", 21),
                text("Back").size(style::font_size::BODY)
            ]
            .spacing(style::spacing::TINY)
            .align_y(iced::Alignment::Center)
        )
        .on_press(Message::Back)
        .padding(8)
        .style(style::secondary_button),
        space().width(Length::Fill),
        // Title
        text(title).size(style::font_size::XLARGE),
        space().width(Length::Fill),
        // Delete button
        button(
            row![
                icons::get_icon("user-trash-full-symbolic", 21).style(|theme, _status| {
                    let danger_style = button::danger(theme, iced::widget::button::Status::Active);
                    iced::widget::svg::Style {
                        color: Some(danger_style.text_color),
                    }
                }),
                text("Delete").size(style::font_size::BODY)
            ]
            .spacing(style::spacing::TINY)
            .align_y(iced::Alignment::Center)
        )
        .style(style::danger_button)
        .padding(8)
        .on_press_maybe(entry.uuid.is_some().then_some(Message::Delete))
    ]
    .spacing(style::spacing::LARGE)
    .padding(20)
    .align_y(iced::Alignment::Center)
    .width(Length::Fill)
    .into()
}

fn upsert_entry_view<'a>(entry: &'a InputableClockodeEntry) -> Element<'a, Message> {
    let button_text = if entry.uuid.is_some() {
        "Update Entry"
    } else {
        "Create Entry"
    };

    let form = column![
        // Name field
        column![
            text("Name")
                .size(style::font_size::BODY)
                .style(style::label_text),
            text_input("e.g., Google Account", &entry.name)
                .on_input(|v| Message::InputUpdated(TOTPEntryInput::UpdateName(v)))
                .padding(12)
                .size(style::font_size::MEDIUM)
        ]
        .spacing(style::spacing::TINY),
        // Secret field
        column![
            text("Secret Key")
                .size(style::font_size::BODY)
                .style(style::label_text),
            text_input("Secret", &entry.secret)
                .on_input(|v| Message::InputUpdated(TOTPEntryInput::UpdateSecret(v)))
                .padding(12)
                .size(style::font_size::MEDIUM)
        ]
        .spacing(style::spacing::TINY),
        // Two column layout for Issuer and Account Name
        row![
            column![
                text("Issuer (Optional)")
                    .size(style::font_size::BODY)
                    .style(style::label_text),
                text_input("e.g., GitHub", entry.issuer.as_deref().unwrap_or(""))
                    .on_input(|v| Message::InputUpdated(TOTPEntryInput::UpdateIssuer(v)))
                    .padding(12)
                    .size(style::font_size::MEDIUM)
            ]
            .spacing(style::spacing::TINY)
            .width(Length::FillPortion(1)),
            column![
                text("Account Name")
                    .size(style::font_size::BODY)
                    .style(style::label_text),
                text_input("e.g., user@example.com", &entry.account_name)
                    .on_input(|v| Message::InputUpdated(TOTPEntryInput::UpdateAccountName(v)))
                    .padding(12)
                    .size(style::font_size::MEDIUM)
            ]
            .spacing(style::spacing::TINY)
            .width(Length::FillPortion(1)),
        ]
        .spacing(style::spacing::MEDIUM),
        // Advanced settings section
        container(
            column![
                text("Advanced Settings").size(style::font_size::MEDIUM),
                // Three column layout for algorithm, digits, and period
                row![
                    column![
                        text("Algorithm")
                            .size(style::font_size::BODY)
                            .style(style::label_text),
                        pick_list(ALL_ALGORITHMS, Some(&entry.algorithm), |v| {
                            Message::InputUpdated(TOTPEntryInput::UpdateAlgorithm(v))
                        })
                        .width(Length::Fill)
                        .padding(12)
                    ]
                    .spacing(style::spacing::TINY)
                    .width(Length::FillPortion(1)),
                    column![
                        text("Digits")
                            .size(style::font_size::BODY)
                            .style(style::label_text),
                        text_input("6 or 8", &entry.digits.to_string())
                            .on_input(|v| Message::InputUpdated(TOTPEntryInput::UpdateDigits(v)))
                            .padding(12)
                            .size(style::font_size::MEDIUM)
                    ]
                    .spacing(style::spacing::TINY)
                    .width(Length::FillPortion(1)),
                    column![
                        text("Period (seconds)")
                            .size(style::font_size::BODY)
                            .style(style::label_text),
                        text_input("30", &entry.step.to_string())
                            .on_input(|v| Message::InputUpdated(TOTPEntryInput::UpdateStep(v)))
                            .padding(12)
                            .size(style::font_size::MEDIUM)
                    ]
                    .spacing(style::spacing::TINY)
                    .width(Length::FillPortion(1)),
                ]
                .spacing(style::spacing::MEDIUM),
            ]
            .spacing(style::spacing::MEDIUM)
        )
        .padding(16)
        .style(style::entry_card),
        // Submit button
        button(
            text(button_text)
                .size(style::font_size::MEDIUM)
                .width(Length::Fill)
                .align_x(Alignment::Center)
        )
        .on_press_maybe(entry.valid().then_some(Message::Submit))
        .padding(16)
        .width(Length::Fill)
        .style(style::primary_submit_button),
    ]
    .spacing(style::spacing::XLARGE)
    .padding(20)
    .max_width(600);

    container(form).center_x(Length::Fill).into()
}
