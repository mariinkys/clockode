// SPDX-License-Identifier: GPL-3.0-only

use iced::{
    Alignment, Element,
    Length::{self},
    Subscription, Task, event,
    keyboard::{self, Key, Modifiers, key::Named},
    time::Instant,
    widget::{
        button, column, container,
        operation::{focus_next, focus_previous},
        pick_list, row, scrollable, space, text, text_input,
    },
};
use rfd::{AsyncFileDialog, FileHandle};
use totp_rs::Algorithm;

use crate::{
    app::{
        core::ClockodeEntry,
        utils::{ALL_ALGORITHMS, InputableClockodeEntry, read_qr_from_file, style},
        widgets::Toast,
    },
    icons,
};

pub struct UpsertPage {
    entry: InputableClockodeEntry,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Callback after pressing a [`Hotkey`] of this page
    Hotkey(Hotkey),
    /// Go back a screen
    Back,

    /// Input update of the various available fields
    InputUpdated(TOTPEntryInput),
    /// Submit the changes
    Submit,
    /// Delete the currently editing entry
    Delete,

    /// Opens the dialog to select a QR file
    OpenQrFileSelection,
    /// Callback after selecting a QR file
    QrFileSelected(Option<FileHandle>),
}

pub enum Action {
    /// Does nothing
    None,
    /// Go back a screen
    Back,
    // Ask parent to run an [`iced::Task`]
    Run(Task<Message>),
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
            Message::Hotkey(hotkey) => match hotkey {
                Hotkey::Tab(modifiers) => {
                    if modifiers.shift() {
                        Action::Run(focus_previous())
                    } else {
                        Action::Run(focus_next())
                    }
                }
            },
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

            Message::OpenQrFileSelection => {
                if self.entry.uuid.is_none() {
                    Action::Run(Task::perform(
                        async move {
                            AsyncFileDialog::new()
                                .add_filter("Image Files", &["png", "jpeg", "jpg", "webp"])
                                .set_directory(dirs::download_dir().unwrap_or("/".into()))
                                .pick_file()
                                .await
                        },
                        Message::QrFileSelected,
                    ))
                } else {
                    Action::None
                }
            }
            Message::QrFileSelected(handle) => {
                if let Some(file_handle) = handle {
                    let result = read_qr_from_file(file_handle.path().to_path_buf());
                    return match result {
                        Ok(value) => {
                            let conv_result = InputableClockodeEntry::try_from(value);
                            match conv_result {
                                Ok(entry) => {
                                    self.entry = entry;
                                    Action::None
                                }
                                Err(e) => Action::AddToast(Toast::error_toast(e)),
                            }
                        }
                        Err(e) => Action::AddToast(Toast::error_toast(e)),
                    };
                }
                Action::None
            }
        }
    }

    pub fn subscription(&self, _now: Instant) -> Subscription<Message> {
        event::listen_with(handle_event)
    }
}

/// View of the header of this screen
fn header_view<'a>(entry: &'a InputableClockodeEntry) -> Element<'a, Message> {
    let (title, subtitle) = if entry.uuid.is_some() {
        ("Edit Entry", "Modify your TOTP entry")
    } else {
        ("New Entry", "Add a new TOTP entry")
    };

    row![
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
        column![
            text(title).size(style::font_size::TITLE),
            text(subtitle)
                .size(style::font_size::SMALL)
                .style(style::muted_text),
        ]
        .spacing(style::spacing::TINY),
        space().width(Length::Fill),
        button(
            row![
                icons::get_icon("qr-symbolic", 21).style(|theme, _status| {
                    let primary_style =
                        button::primary(theme, iced::widget::button::Status::Active);
                    iced::widget::svg::Style {
                        color: Some(primary_style.text_color),
                    }
                }),
                text("QR (File)").size(style::font_size::BODY)
            ]
            .spacing(style::spacing::TINY)
            .align_y(iced::Alignment::Center)
        )
        .style(style::primary_button)
        .padding(8)
        .on_press_maybe(entry.uuid.is_none().then_some(Message::OpenQrFileSelection)),
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
    .padding(10)
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
    .padding(10)
    .max_width(600);

    scrollable(container(form).center_x(Length::Fill))
        .width(Length::Fill)
        .into()
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
