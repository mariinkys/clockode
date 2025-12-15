// SPDX-License-Identifier: GPL-3.0-only

use std::sync::{Arc, Mutex};

use iced::{
    Alignment, Element,
    Length::{self},
    Subscription, Task, Theme,
    time::Instant,
    widget::{button, column, container, pick_list, row, scrollable, space, text},
};

use crate::{
    APP_ID,
    app::{utils::style, widgets::Toast},
    config::{ColockodeTheme, Config},
    icons,
};

pub struct SettingsPage {
    config: Arc<Mutex<Config>>,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Go back a screen
    Back,
    /// Callback after the user changes the current theme
    ChangedTheme(ColockodeTheme),
    /// Configuration Saved
    ConfigurationSaved(Result<(), anywho::Error>),
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
}

impl SettingsPage {
    pub fn new(config: Arc<Mutex<Config>>) -> (Self, Task<Message>) {
        (Self { config }, Task::none())
    }

    pub fn view(&self, _now: Instant) -> iced::Element<'_, Message> {
        let header = header_view();
        let content = settings_view(&self.config);

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
            Message::ChangedTheme(colockode_theme) => {
                if let Ok(mut cfg) = self.config.lock() {
                    cfg.theme = colockode_theme.clone();
                    let cfg_clone = cfg.clone();

                    return Action::Run(Task::perform(
                        async move { cfg_clone.save(APP_ID).await },
                        Message::ConfigurationSaved,
                    ));
                } else {
                    eprintln!("Warning: config mutex poisoned. Cannot change theme.");
                }
                Action::None
            }
            Message::ConfigurationSaved(result) => match result {
                Ok(_) => Action::None,
                Err(e) => Action::AddToast(Toast::error_toast(e)),
            },
        }
    }

    pub fn subscription(&self, _now: Instant) -> Subscription<Message> {
        Subscription::none()
    }
}

/// View of the header of this screen
fn header_view<'a>() -> Element<'a, Message> {
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
        column![
            text("Settings").size(style::font_size::TITLE),
            text("Application preferences")
                .size(style::font_size::SMALL)
                .style(style::muted_text),
        ]
        .spacing(style::spacing::TINY),
        space().width(Length::Fill),
    ]
    .spacing(style::spacing::LARGE)
    .padding(20)
    .align_y(iced::Alignment::Center)
    .width(Length::Fill)
    .into()
}

fn settings_view<'a>(config: &'a Arc<Mutex<Config>>) -> Element<'a, Message> {
    let settings_form = column![
        // Export and Import buttons in a row
        column![
            text("Vault Management")
                .size(style::font_size::BODY)
                .style(style::label_text),
            row![
                button(
                    row![
                        icons::get_icon("document-save-symbolic", 21),
                        text("Export").size(style::font_size::BODY)
                    ]
                    .spacing(style::spacing::TINY)
                    .align_y(Alignment::Center)
                )
                //.on_press(Message::ExportVault)
                .padding(12)
                .width(Length::Fill)
                .style(style::primary_button),
                button(
                    row![
                        icons::get_icon("document-open-symbolic", 21),
                        text("Import").size(style::font_size::BODY)
                    ]
                    .spacing(style::spacing::TINY)
                    .align_y(Alignment::Center)
                )
                //.on_press(Message::ImportVault)
                .padding(12)
                .width(Length::Fill)
                .style(style::primary_button),
            ]
            .spacing(style::spacing::MEDIUM),
        ]
        .spacing(style::spacing::TINY),
        // Theme picker
        column![
            text("Theme")
                .size(style::font_size::BODY)
                .style(style::label_text),
            pick_list(
                Theme::ALL,
                Some::<Theme>({
                    let cfg = config.lock().map(|c| c.theme.clone()).unwrap_or_default();
                    cfg.into()
                }),
                |t| { Message::ChangedTheme(ColockodeTheme::try_from(&t).unwrap_or_default()) }
            )
            .width(Length::Fill)
            .padding(12)
        ]
        .spacing(style::spacing::TINY),
    ]
    .spacing(style::spacing::XLARGE)
    .padding(20)
    .max_width(600);

    container(
        column![
            scrollable(container(settings_form).center_x(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill),
            // App version at the bottom
            container(
                text(format!("Version {}", env!("CARGO_PKG_VERSION")))
                    .size(style::font_size::SMALL)
                    .style(style::muted_text)
            )
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .padding(20),
        ]
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
