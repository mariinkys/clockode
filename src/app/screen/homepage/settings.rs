// SPDX-License-Identifier: GPL-3.0-only

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use iced::{
    Alignment, Element,
    Length::{self},
    Subscription, Task, Theme,
    time::Instant,
    widget::{button, column, container, pick_list, row, scrollable, space, text},
};
use rfd::{AsyncFileDialog, FileHandle};

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
    /// Open the File Dialog to select a file to import
    OpenImportDialog,
    /// Open the File Dialog to select where to export the file
    OpenExportDialog,
    /// Import Path Selected Callback (after dialog)
    ImportPathSelected(Option<FileHandle>),
    /// Export Path Selected Callback (after dialog)
    ExportPathSelected(Option<FileHandle>),
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
    /// Ask parent to import some content from the given filepath
    ImportContent(PathBuf),
    /// Ask parent to export the context to the given filepath
    ExportContent(PathBuf),
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
            Message::OpenImportDialog => Action::Run(Task::perform(
                async move {
                    AsyncFileDialog::new()
                        .add_filter("txt", &["txt"])
                        .set_directory(dirs::download_dir().unwrap_or("/".into()))
                        .pick_file()
                        .await
                },
                Message::ImportPathSelected,
            )),
            Message::OpenExportDialog => Action::Run(Task::perform(
                async move {
                    AsyncFileDialog::new()
                        .set_file_name("export.txt")
                        .set_directory(dirs::download_dir().unwrap_or("/".into()))
                        .save_file()
                        .await
                },
                Message::ExportPathSelected,
            )),
            Message::ImportPathSelected(handle) => {
                if let Some(file_handle) = handle {
                    return Action::ImportContent(file_handle.path().to_path_buf());
                }
                Action::None
            }
            Message::ExportPathSelected(handle) => {
                if let Some(file_handle) = handle {
                    return Action::ExportContent(file_handle.path().to_path_buf());
                }
                Action::None
            }
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
    .padding(10)
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
                        icons::get_icon("document-export-symbolic", 21).style(|theme, _status| {
                            let primary_style =
                                button::primary(theme, iced::widget::button::Status::Active);
                            iced::widget::svg::Style {
                                color: Some(primary_style.text_color),
                            }
                        }),
                        text("Export").size(style::font_size::MEDIUM)
                    ]
                    .spacing(style::spacing::TINY)
                    .align_y(Alignment::Center)
                )
                .on_press(Message::OpenExportDialog)
                .padding(12)
                .width(Length::Fill)
                .style(style::primary_button),
                button(
                    row![
                        icons::get_icon("document-import-symbolic", 21).style(|theme, _status| {
                            let primary_style =
                                button::primary(theme, iced::widget::button::Status::Active);
                            iced::widget::svg::Style {
                                color: Some(primary_style.text_color),
                            }
                        }),
                        text("Import").size(style::font_size::MEDIUM)
                    ]
                    .spacing(style::spacing::TINY)
                    .align_y(Alignment::Center)
                )
                .on_press(Message::OpenImportDialog)
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
    .padding(10)
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
            .padding(10),
        ]
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
