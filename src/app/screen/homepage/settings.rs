// SPDX-License-Identifier: GPL-3.0-only

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use iced::{
    Alignment, Color, Element,
    Length::{self},
    Subscription, Task, Theme, Widget, event,
    keyboard::{self, Key, key::Named},
    time::Instant,
    widget::{
        Column, Row, button, center, column, container, mouse_area, opaque, operation, pick_list,
        row, scrollable, space, stack, text, text_input,
    },
};
use rfd::{AsyncFileDialog, FileHandle};
use secrecy::SecretString;
use tracing::error;

use crate::{
    APP_ID,
    app::{
        core::{ClockodeEntry, specific_impl::aegis},
        utils::{ImportType, style},
        widgets::{Toast, menu_button::menu_button},
    },
    config::{ColockodeTheme, Config},
    icons,
};

const AEGIS_PASSWORD_INPUT: &str = "settings-aegis-password";

pub struct SettingsPage {
    config: Arc<Mutex<Config>>,
    show_shortcuts: bool,
    aegis_import: Option<AegisImport>,
}

struct AegisImport {
    path: PathBuf,
    password: String,
    error: Option<String>,
    decrypting: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Go back a screen
    Back,
    /// Callback after pressing a [`Hotkey`] of this page
    Hotkey(Hotkey),
    /// Callback after the user changes the current theme
    ChangedTheme(ColockodeTheme),
    /// Configuration Saved
    ConfigurationSaved(Result<(), anywho::Error>),
    /// Open the File Dialog to select a file to import
    OpenImportDialog(ImportType),
    /// Open the File Dialog to select where to export the file
    OpenExportDialog,
    /// Import Path Selected Callback (after dialog)
    ImportPathSelected(Option<FileHandle>),
    /// Export Path Selected Callback (after dialog)
    ExportPathSelected(Option<FileHandle>),
    /// Opens the given URL in the browser
    LaunchUrl(String),
    /// Open the keyboard shortcuts dialog
    ShowShortcuts,
    /// Close the keyboard shortcuts dialog
    HideShortcuts,
    /// A path for an Aegis import has been selected on the dialog
    AegisPathSelected(Option<FileHandle>),
    /// Update the state of the password field for the aegis import
    AegisPasswordChanged(String),
    /// Submit import with current password
    SubmitAegisImport,
    /// Cancel current aegis import
    CancelAegisImport,
    /// Result after trying to decrypt the aegis import
    AegisDecrypted(Result<Vec<ClockodeEntry>, anywho::Error>),
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
    /// Ask parent to import some entries
    ImportEntries(Vec<ClockodeEntry>),
    /// Ask parent to export the context to the given filepath
    ExportContent(PathBuf),
}

impl SettingsPage {
    pub fn new(config: Arc<Mutex<Config>>) -> (Self, Task<Message>) {
        (
            Self {
                config,
                show_shortcuts: false,
                aegis_import: None,
            },
            Task::none(),
        )
    }

    pub fn view(&self, _now: Instant) -> impl Widget<Message> {
        let header = header_view();
        let content = settings_view(&self.config);

        let page = container(
            container(column![header, content])
                .padding(5.)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .center(Length::Fill);

        if let Some(import) = &self.aegis_import {
            modal(
                page,
                aegis_password_view(import),
                Message::CancelAegisImport,
            )
            .boxed()
        } else if self.show_shortcuts {
            modal(page, shortcuts_view(), Message::HideShortcuts).boxed()
        } else {
            page.boxed()
        }
    }

    pub fn update(&mut self, message: Message, _now: Instant) -> Action {
        match message {
            Message::Back => Action::Back,
            Message::Hotkey(hotkey) => match hotkey {
                Hotkey::Esc if self.aegis_import.is_some() => {
                    self.aegis_import = None;
                    Action::None
                }
                Hotkey::Esc if self.show_shortcuts => {
                    self.show_shortcuts = false;
                    Action::None
                }
                Hotkey::Esc => Action::Back,
            },
            Message::ChangedTheme(colockode_theme) => {
                if let Ok(mut cfg) = self.config.lock() {
                    cfg.theme = colockode_theme.clone();
                    let cfg_clone = cfg.clone();

                    return Action::Run(Task::perform(
                        async move { cfg_clone.save(APP_ID).await },
                        Message::ConfigurationSaved,
                    ));
                } else {
                    error!("Warning: config mutex poisoned. Cannot change theme.");
                }
                Action::None
            }
            Message::ConfigurationSaved(result) => match result {
                Ok(_) => Action::None,
                Err(e) => Action::AddToast(Toast::error_toast(e)),
            },
            Message::OpenImportDialog(import_type) => match import_type {
                ImportType::Standard => Action::Run(Task::perform(
                    async move {
                        AsyncFileDialog::new()
                            .add_filter("txt", &["txt"])
                            .set_directory(dirs::download_dir().unwrap_or("/".into()))
                            .pick_file()
                            .await
                    },
                    Message::ImportPathSelected,
                )),
                ImportType::AegisEncrypted => Action::Run(Task::perform(
                    async move {
                        AsyncFileDialog::new()
                            .add_filter("json", &["json"])
                            .set_directory(dirs::download_dir().unwrap_or("/".into()))
                            .pick_file()
                            .await
                    },
                    Message::AegisPathSelected,
                )),
            },
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
            Message::LaunchUrl(url) => {
                match open::that_detached(&url) {
                    Ok(()) => {}
                    Err(err) => {
                        error!("failed to open {url:?}: {err}");
                    }
                }
                Action::None
            }
            Message::ShowShortcuts => {
                self.show_shortcuts = true;
                Action::None
            }
            Message::HideShortcuts => {
                self.show_shortcuts = false;
                Action::None
            }
            Message::AegisPathSelected(handle) => {
                let Some(handle) = handle else {
                    return Action::None;
                };

                self.aegis_import = Some(AegisImport {
                    path: handle.path().to_path_buf(),
                    password: String::new(),
                    error: None,
                    decrypting: false,
                });
                Action::Run(operation::focus(AEGIS_PASSWORD_INPUT))
            }
            Message::AegisPasswordChanged(password) => {
                if let Some(import) = &mut self.aegis_import {
                    import.password = password;
                    import.error = None;
                }
                Action::None
            }
            Message::SubmitAegisImport => {
                let Some(import) = &mut self.aegis_import else {
                    return Action::None;
                };
                if import.decrypting || import.password.is_empty() {
                    return Action::None;
                }

                import.decrypting = true;
                let path = import.path.clone();
                let password = SecretString::from(import.password.clone());

                Action::Run(Task::perform(
                    aegis::import(path, password),
                    Message::AegisDecrypted,
                ))
            }
            Message::AegisDecrypted(result) => {
                let Some(import) = &mut self.aegis_import else {
                    return Action::None;
                };

                match result {
                    Ok(entries) => {
                        self.aegis_import = None;
                        Action::ImportEntries(entries)
                    }
                    Err(err) => {
                        import.decrypting = false;
                        import.error = Some(err.to_string());
                        Action::None
                    }
                }
            }
            Message::CancelAegisImport => {
                self.aegis_import = None;
                Action::None
            }
        }
    }

    pub fn subscription(&self, _now: Instant) -> Subscription<Message> {
        event::listen_with(handle_event)
    }
}

/// View of the header of this screen
fn header_view() -> impl Widget<Message> {
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
}

fn settings_view(config: &Arc<Mutex<Config>>) -> impl Widget<Message> {
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
                menu_button(
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
                    .align_y(Alignment::Center),
                    ImportType::ALL,
                    ImportType::to_string
                )
                .on_select(Message::OpenImportDialog)
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
                {
                    let cfg = config.lock().map(|c| c.theme.clone()).unwrap_or_default();
                    let theme: Theme = cfg.into();
                    Some(theme)
                },
                Theme::ALL,
                |t: &Theme| t.to_string(),
            )
            .on_select(|t| Message::ChangedTheme(ColockodeTheme::try_from(&t).unwrap_or_default()))
            .width(Length::Fill)
            .padding(12)
        ]
        .spacing(style::spacing::TINY),
        // Keyboard shortcuts
        container(
            button(
                row![
                    icons::get_icon("input-keyboard-symbolic", 16).style(style::icon_muted),
                    text("Keyboard Shortcuts").size(style::font_size::SMALL)
                ]
                .spacing(style::spacing::SMALL)
                .align_y(Alignment::Center)
            )
            .on_press(Message::ShowShortcuts)
            .padding([6, 14])
            .style(style::ghost_button),
        )
        .center_x(Length::Fill),
    ]
    .spacing(style::spacing::XLARGE)
    .padding(10)
    .width(Length::Fill.max(600));

    container(
        column![
            scrollable(container(settings_form).center_x(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill),
            // App version at the bottom
            container(
                column![
                    iced::widget::iced(15.),
                    row![
                        mouse_area(
                            text(format!("Version {}", env!("CARGO_PKG_VERSION")))
                                .size(style::font_size::SMALL)
                                .style(style::link_text)
                        )
                        .on_press(Message::LaunchUrl(String::from(
                            "https://github.com/mariinkys/clockode/releases"
                        ))),
                        text(" - ")
                            .size(style::font_size::SMALL)
                            .style(style::muted_text),
                        mouse_area(
                            text("Donate")
                                .size(style::font_size::SMALL)
                                .style(style::link_text)
                        )
                        .on_press(Message::LaunchUrl(String::from(
                            "https://buymeacoffee.com/mariinkys"
                        )))
                    ]
                    .spacing(style::spacing::SMALL)
                ]
                .spacing(style::spacing::SMALL)
                .align_x(Alignment::Center)
                .width(Length::Shrink),
            )
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .padding(10),
        ]
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
}

/// The Aegis vault password dialog card
fn aegis_password_view(import: &AegisImport) -> impl Widget<Message> {
    let submit = (!import.decrypting).then_some(Message::SubmitAegisImport);

    container(
        column![
            text("Aegis Vault Password").size(style::font_size::LARGE),
            text("Enter the password used to encrypt this vault")
                .size(style::font_size::SMALL)
                .style(style::muted_text),
            text_input("Password", &import.password)
                .id(AEGIS_PASSWORD_INPUT)
                .secure(true)
                .on_input(Message::AegisPasswordChanged)
                .on_submit(Message::SubmitAegisImport)
                .padding(8)
                .style(style::text_input_style),
            text(import.error.as_deref().unwrap_or(""))
                .size(style::font_size::SMALL)
                .style(text::danger),
            row![
                space().width(Length::Fill),
                button(text("Cancel"))
                    .on_press(Message::CancelAegisImport)
                    .padding(8)
                    .style(style::secondary_button),
                button(text(if import.decrypting {
                    "Decrypting..."
                } else {
                    "Import"
                }))
                .on_press_maybe(submit)
                .padding(8)
                .style(style::primary_button),
            ]
            .spacing(style::spacing::SMALL),
        ]
        .spacing(style::spacing::MEDIUM),
    )
    .padding(20)
    .width(Length::Fill.max(420.0))
    .style(style::card_container)
}

//
// SUBSCRIPTIONS
//

#[derive(Debug, Clone)]
pub enum Hotkey {
    Esc,
}

fn handle_event(event: event::Event, _: event::Status, _: iced::window::Id) -> Option<Message> {
    #[allow(clippy::collapsible_match)]
    match event {
        event::Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => match key {
            Key::Named(Named::Escape) => Some(Message::Hotkey(Hotkey::Esc)),
            _ => None,
        },
        _ => None,
    }
}

//
// KEYBOARD SHORTCUTS
//

/// A keyboard shortcut: one or more alternative key combos and what they do
struct Shortcut {
    /// Alternatives (shown separated by "or"), each a combo of keys (joined by "+")
    keys: &'static [&'static [&'static str]],
    description: &'static str,
}

const SHORTCUT_SECTIONS: &[(&str, &[Shortcut])] = &[
    (
        "Home Screen",
        &[
            Shortcut {
                keys: &[&["Tab"]],
                description: "Highlight Next Entry",
            },
            Shortcut {
                keys: &[&["Shift", "Tab"]],
                description: "Highlight Previous Entry",
            },
            Shortcut {
                keys: &[&["Enter"], &["C"]],
                description: "Copy Highlighted Code",
            },
            Shortcut {
                keys: &[&["E"]],
                description: "Edit Highlighted Entry",
            },
            Shortcut {
                keys: &[&["S"]],
                description: "Open Search",
            },
            Shortcut {
                keys: &[&["Esc"]],
                description: "Close Search or Clear Highlight",
            },
        ],
    ),
    (
        "Add & Edit Entry Screens",
        &[
            Shortcut {
                keys: &[&["Tab"]],
                description: "Next Field",
            },
            Shortcut {
                keys: &[&["Shift", "Tab"]],
                description: "Previous Field",
            },
            Shortcut {
                keys: &[&["Esc"]],
                description: "Go Back",
            },
        ],
    ),
    (
        "Settings Screen",
        &[Shortcut {
            keys: &[&["Esc"]],
            description: "Go Back",
        }],
    ),
];

/// The keyboard shortcuts dialog card
fn shortcuts_view() -> impl Widget<Message> {
    let title = row![
        text("Keyboard Shortcuts")
            .size(style::font_size::LARGE)
            .font(iced::Font {
                weight: iced::font::Weight::ExtraBold,
                ..iced::Font::DEFAULT
            })
            .width(Length::Fill),
        button(icons::get_icon("window-close-symbolic", 16).style(style::icon))
            .on_press(Message::HideShortcuts)
            .padding(4)
            .style(style::transparent_button),
    ]
    .align_y(Alignment::Center);

    let sections = SHORTCUT_SECTIONS
        .iter()
        .map(|(name, shortcuts)| -> Element<'_, Message> {
            column![
                text(*name)
                    .size(style::font_size::SMALL)
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..iced::Font::DEFAULT
                    })
                    .style(style::label_text),
                Column::with_children(shortcuts.iter().map(shortcut_row))
                    .spacing(style::spacing::SMALL),
            ]
            .spacing(style::spacing::SMALL)
            .boxed()
        });

    container(
        column![
            title,
            scrollable(Column::with_children(sections).spacing(style::spacing::LARGE)),
        ]
        .spacing(style::spacing::MEDIUM),
    )
    .padding(20)
    .width(Length::Fill.max(420.0))
    .style(style::card_container)
}

/// One line of the dialog: description on the left, keycaps on the right
fn shortcut_row<'a>(shortcut: &Shortcut) -> Element<'a, Message> {
    let mut keys: Row<Element<'a, Message>> = Row::new()
        .spacing(style::spacing::TINY)
        .align_y(Alignment::Center);

    for (i, combo) in shortcut.keys.iter().enumerate() {
        if i > 0 {
            keys = keys.push(
                text("or")
                    .size(style::font_size::SMALL)
                    .style(style::muted_text)
                    .boxed(),
            );
        }

        for (j, key) in combo.iter().enumerate() {
            if j > 0 {
                keys = keys.push(
                    text("+")
                        .size(style::font_size::SMALL)
                        .style(style::muted_text)
                        .boxed(),
                );
            }

            keys = keys.push(
                container(
                    text(*key)
                        .size(style::font_size::SMALL)
                        .font(iced::Font::MONOSPACE),
                )
                .padding([2, 8])
                .style(style::keycap)
                .boxed(),
            );
        }
    }

    row![
        text(shortcut.description)
            .size(style::font_size::BODY)
            .width(Length::Fill),
        keys,
    ]
    .spacing(style::spacing::MEDIUM)
    .align_y(Alignment::Center)
    .boxed()
}

/// Shows `content` centered over a dimmed `base`; clicking the backdrop sends `on_blur`
fn modal<'a>(
    base: impl Widget<Message> + 'a,
    content: impl Widget<Message> + 'a,
    on_blur: Message,
) -> impl Widget<Message> {
    stack![
        base,
        opaque(
            mouse_area(
                center(opaque(container(content).padding(style::spacing::LARGE))).style(|_theme| {
                    container::Style {
                        background: Some(
                            Color {
                                a: 0.6,
                                ..Color::BLACK
                            }
                            .into(),
                        ),
                        ..container::Style::default()
                    }
                })
            )
            .on_press(on_blur)
        )
    ]
}
