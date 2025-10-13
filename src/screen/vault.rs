// SPDX-License-Identifier: GPL-3.0-only

use arboard::Clipboard;
use iced::keyboard::key::Named;
use iced::keyboard::{self, Key, Modifiers};
use iced::time::Instant;
use iced::widget::operation::{focus_next, focus_previous};
use iced::widget::{
    button, column, container, float, mouse_area, pick_list, row,
    scrollable, text, text_input, tooltip,
};
use iced::{Alignment, Element, Length, Padding, Subscription, Task, Theme, event};
use rfd::AsyncFileDialog;
use std::collections::HashMap;

use crate::config::{ColockodeTheme, Config};
use crate::core::entry::{self, Algorithm, Entry, TOTPConfig};
use crate::widgets::toast::Toast;
use crate::{icons, style::*};

pub struct Vault {
    state: State,
    vault: Option<crate::Vault>,
    clipboard: Option<Clipboard>,
    config: Config,
}

#[derive(Debug, Clone)]
pub enum Message {
    Hotkey(Hotkey),
    SetClipboardContent(String),
    ChangedTheme(ColockodeTheme),
    TextInputted(TextInputs, String),

    CreateVault,
    CreatedVault(Result<crate::Vault, anywho::Error>),

    UnlockVault,
    UnlockedVault((crate::Vault, Option<anywho::Error>)),
    SavedVault(Result<(), anywho::Error>),
    OpenExportVaultDialog(ExportImportType),
    ExportVault(Box<Option<rfd::FileHandle>>, ExportImportType),
    ExportedVault(Result<String, anywho::Error>),
    OpenImportVaultDialog(ExportImportType),
    ImportVault(Box<Option<rfd::FileHandle>>, ExportImportType),
    ImportedVault(Result<HashMap<entry::Id, Entry>, anywho::Error>),

    OpenModal(Modal),

    UpsertEntry(Entry),
    DeleteEntry(entry::Id),
    UpdateSelectedAlgorithm(Algorithm),
    ToggleAdvancedConfig,

    UpdateAllTOTP,
    UpdatedAllTOTP(Result<HashMap<entry::Id, Entry>, anywho::Error>),
    UpdateTimeCount,
}

pub enum State {
    Creation {
        new_password: String,
        new_password_repeat: String,
    },
    Decryption {
        password: String,
    },
    List {
        time_count: u64,
        modal: Modal,
    },
}

pub enum Action {
    None,
    Run(Task<Message>),
    AddToast(Toast),
    ChangedTheme(ColockodeTheme),
}

#[derive(Debug, Clone)]
pub enum TextInputs {
    NewPassword,
    NewPasswordRepeat,
    Password,

    EntryName,
    EntrySecret,

    EntryConfigDigits,
    EntryConfigSkew,
}

#[derive(Debug, Clone)]
pub enum Modal {
    None,
    AddEdit {
        entry_id: Option<entry::Id>,
        entry_name: String,
        entry_secret: String,
        entry_config: TOTPConfig,
        show_advanced: bool,
        next_input_clean: bool,
    },
    Config,
}

impl Modal {
    pub fn close() -> Modal {
        Modal::None
    }

    pub fn add_edit(entry: Option<Entry>) -> Modal {
        match entry {
            Some(entry) => Modal::AddEdit {
                entry_id: entry.id,
                entry_name: entry.name,
                entry_secret: entry.secret,
                entry_config: entry.totp_config,
                show_advanced: false,
                next_input_clean: false,
            },
            None => Modal::AddEdit {
                entry_id: None,
                entry_name: String::new(),
                entry_secret: String::new(),
                entry_config: TOTPConfig::default(),
                show_advanced: false,
                next_input_clean: false,
            },
        }
    }

    pub fn config() -> Modal {
        Modal::Config
    }
}

#[derive(Debug, Clone)]
pub enum ExportImportType {
    Custom,
    Standard,
}

impl Vault {
    const APP_TITLE: &str = "Clockode";
    const REFRESH_RATE: u64 = 30;

    pub fn new(vault: Result<crate::Vault, anywho::Error>, config: Config) -> Self {
        let clipboard = Clipboard::new();
        if let Err(clip_err) = &clipboard {
            eprintln!("{clip_err}");
        };

        if let Ok(vault) = vault {
            Self {
                state: State::Decryption {
                    password: String::new(),
                },
                vault: Some(vault),
                clipboard: clipboard.ok(),
                config,
            }
        } else {
            Self {
                state: State::Creation {
                    new_password: String::new(),
                    new_password_repeat: String::new(),
                },
                vault: None,
                clipboard: clipboard.ok(),
                config,
            }
        }
    }

    pub fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    #[allow(clippy::only_used_in_recursion)]
    pub fn update(&mut self, message: Message, now: Instant) -> Action {
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
            Message::SetClipboardContent(content) => {
                if let Some(clipboard) = &mut self.clipboard {
                    let res = &clipboard.set_text(content);
                    match res {
                        Ok(_) => {
                            return Action::AddToast(Toast::success_toast("Copied to clipboard"));
                        }
                        Err(err) => {
                            eprintln!("{err}");
                        }
                    }
                }
                Action::None
            }
            Message::ChangedTheme(theme) => {
                self.config.theme = theme.clone();
                Action::ChangedTheme(theme)
            }
            Message::TextInputted(text_inputs, value) => {
                match text_inputs {
                    TextInputs::NewPassword => {
                        if let State::Creation { new_password, .. } = &mut self.state {
                            *new_password = value;
                        }
                    }
                    TextInputs::NewPasswordRepeat => {
                        if let State::Creation {
                            new_password_repeat,
                            ..
                        } = &mut self.state
                        {
                            *new_password_repeat = value;
                        }
                    }
                    TextInputs::Password => {
                        if let State::Decryption { password, .. } = &mut self.state {
                            *password = value;
                        }
                    }
                    TextInputs::EntryName =>
                    {
                        #[allow(clippy::collapsible_match)]
                        if let State::List { modal, .. } = &mut self.state {
                            if let Modal::AddEdit { entry_name, .. } = modal {
                                *entry_name = value;
                            }
                        }
                    }
                    TextInputs::EntrySecret =>
                    {
                        #[allow(clippy::collapsible_match)]
                        if let State::List { modal, .. } = &mut self.state {
                            if let Modal::AddEdit { entry_secret, .. } = modal {
                                *entry_secret = value;
                            }
                        }
                    }
                    TextInputs::EntryConfigDigits => {
                        #[allow(clippy::collapsible_match)]
                        if let State::List { modal, .. } = &mut self.state {
                            if let Modal::AddEdit {
                                entry_config,
                                next_input_clean,
                                ..
                            } = modal
                            {
                                if value.is_empty() {
                                    entry_config.digits = 0;
                                    *next_input_clean = true;
                                } else if *next_input_clean {
                                    let new_value = value.replace("0", "");
                                    let new_value = new_value.trim();
                                    entry_config.digits = new_value.parse::<u32>().unwrap_or(6);
                                    *next_input_clean = false;
                                } else {
                                    entry_config.digits = value.parse::<u32>().unwrap_or(6);
                                }
                            }
                        }
                    }
                    TextInputs::EntryConfigSkew => {
                        #[allow(clippy::collapsible_match)]
                        if let State::List { modal, .. } = &mut self.state {
                            if let Modal::AddEdit {
                                entry_config,
                                next_input_clean,
                                ..
                            } = modal
                            {
                                if value.is_empty() {
                                    entry_config.skew = 0;
                                    *next_input_clean = true;
                                } else if *next_input_clean {
                                    let new_value = value.replace("0", "");
                                    let new_value = new_value.trim();
                                    entry_config.skew = new_value.parse::<u8>().unwrap_or(1);
                                    *next_input_clean = false;
                                } else {
                                    entry_config.skew = value.parse::<u8>().unwrap_or(1);
                                }
                            }
                        }
                    }
                }
                Action::None
            }
            Message::CreateVault => {
                if let State::Creation { new_password, .. } = &mut self.state {
                    let password = std::mem::take(new_password);

                    Action::Run(Task::perform(
                        crate::Vault::create(password),
                        Message::CreatedVault,
                    ))
                } else {
                    Action::None
                }
            }
            Message::CreatedVault(res) => {
                match res {
                    Ok(vault) => {
                        self.state = State::Decryption {
                            password: String::new(),
                        };
                        self.vault = Some(vault);
                    }
                    Err(err) => {
                        eprintln!("{err}");
                    }
                }
                Action::None
            }
            Message::UnlockVault => {
                if self.vault.is_some() {
                    let vault = self.vault.take().unwrap();

                    if let State::Decryption { password, .. } = &mut self.state {
                        return Action::Run(Task::perform(
                            vault.decrypt(password.to_string()),
                            Message::UnlockedVault,
                        ));
                    }
                }

                Action::None
            }
            Message::UnlockedVault((vault, error)) => match error {
                None => {
                    self.state = State::List {
                        modal: Modal::None,
                        time_count: get_time_until_next_totp_refresh(Self::REFRESH_RATE),
                    };
                    self.vault = Some(vault);
                    self.update(Message::UpdateAllTOTP, now)
                }
                Some(err) => {
                    eprintln!("{err}");
                    self.vault = Some(vault);
                    Action::AddToast(Toast::error_toast(format!("{err}")))
                }
            },
            Message::SavedVault(res) => {
                match res {
                    Ok(_) => {
                        if let State::List { modal, .. } = &mut self.state {
                            *modal = Modal::close();
                        }
                    }
                    Err(err) => {
                        eprintln!("Error saving vault: {err}");
                    }
                }
                Action::None
            }
            Message::OpenExportVaultDialog(export_type) => match export_type {
                ExportImportType::Custom => Action::Run(Task::perform(
                    async move {
                        let result = AsyncFileDialog::new()
                            .set_file_name("vault_export.ron")
                            .set_directory(dirs::download_dir().unwrap_or("/".into()))
                            .save_file()
                            .await;

                        Box::new(result)
                    },
                    |res| Message::ExportVault(res, ExportImportType::Custom),
                )),
                ExportImportType::Standard => Action::Run(Task::perform(
                    async move {
                        let result = AsyncFileDialog::new()
                            .set_file_name("vault_export.txt")
                            .set_directory(dirs::download_dir().unwrap_or("/".into()))
                            .save_file()
                            .await;

                        Box::new(result)
                    },
                    |res| Message::ExportVault(res, ExportImportType::Standard),
                )),
            },
            Message::ExportVault(handle, export_type) => {
                if let Some(file_handle) = *handle {
                    if let Some(vault) = &self.vault {
                        match vault.entries() {
                            Some(entries) => {
                                if entries.is_empty() {
                                    return Action::None;
                                }

                                let cloned_vault = vault.clone(); // CLONE
                                return match export_type {
                                    ExportImportType::Custom => Action::Run(Task::perform(
                                        async move { cloned_vault.export(file_handle.path()).await },
                                        Message::ExportedVault,
                                    )),
                                    ExportImportType::Standard => Action::Run(Task::perform(
                                        async move { cloned_vault.export_uri(file_handle.path()).await },
                                        Message::ExportedVault,
                                    )),
                                };
                            }
                            None => {
                                println!("Error getting vault entries");
                            }
                        }
                    }
                }
                Action::None
            }
            Message::ExportedVault(res) => {
                match res {
                    Ok(path) => {
                        println!("Vault exported sucessfully to: {path}");
                        return Action::AddToast(Toast::success_toast(format!(
                            "Vault exported sucessfully to: {path}"
                        )));
                    }
                    Err(err) => {
                        eprintln!("{err}");
                    }
                }
                Action::None
            }
            Message::OpenImportVaultDialog(import_type) => match import_type {
                ExportImportType::Custom => Action::Run(Task::perform(
                    async move {
                        let result = AsyncFileDialog::new()
                            .add_filter("ron", &["ron"])
                            .set_directory(dirs::download_dir().unwrap_or("/".into()))
                            .pick_file()
                            .await;

                        Box::new(result)
                    },
                    |res| Message::ImportVault(res, ExportImportType::Custom),
                )),
                ExportImportType::Standard => Action::Run(Task::perform(
                    async move {
                        let result = AsyncFileDialog::new()
                            .add_filter("txt", &["txt"])
                            .set_directory(dirs::download_dir().unwrap_or("/".into()))
                            .pick_file()
                            .await;

                        Box::new(result)
                    },
                    |res| Message::ImportVault(res, ExportImportType::Standard),
                )),
            },
            Message::ImportVault(handle, import_type) => {
                if let Some(file_handle) = *handle {
                    if let Some(vault) = &mut self.vault {
                        let mut cloned_vault = vault.clone(); // CLONE
                        return match import_type {
                            ExportImportType::Custom => Action::Run(Task::perform(
                                async move {
                                    cloned_vault.import(file_handle.path().to_path_buf()).await
                                },
                                Message::ImportedVault,
                            )),
                            ExportImportType::Standard => Action::Run(Task::perform(
                                async move {
                                    cloned_vault
                                        .import_uri(file_handle.path().to_path_buf())
                                        .await
                                },
                                Message::ImportedVault,
                            )),
                        };
                    }
                }
                Action::None
            }
            Message::ImportedVault(new_entries) => {
                match new_entries {
                    Ok(new_entries) => {
                        if let Some(vault) = &mut self.vault {
                            let res = vault.add_entries(new_entries, Self::REFRESH_RATE);
                            match res {
                                Ok(_) => {
                                    let cloned_vault = vault.clone(); // CLONE
                                    return Action::Run(Task::perform(
                                        async move { cloned_vault.save().await },
                                        Message::SavedVault,
                                    ));
                                }
                                Err(err) => {
                                    eprintln!("{err}");
                                    return Action::None;
                                }
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("{err}");
                    }
                }

                Action::None
            }
            Message::OpenModal(new_modal) => {
                if let State::List { modal, .. } = &mut self.state {
                    *modal = new_modal;
                }
                Action::None
            }
            Message::UpsertEntry(entry) => {
                if let Some(vault) = &mut self.vault {
                    let res = vault.upsert_entry(entry, Self::REFRESH_RATE);
                    match res {
                        Ok(_) => {
                            let cloned_vault = vault.clone(); // CLONE
                            return Action::Run(Task::perform(
                                async move { cloned_vault.save().await },
                                Message::SavedVault,
                            ));
                        }
                        Err(err) => {
                            eprintln!("{err}");
                            return Action::None;
                        }
                    }
                }

                Action::None
            }
            Message::DeleteEntry(entry_id) => {
                if let Some(vault) = &mut self.vault {
                    let res = vault.delete_entry(entry_id);
                    match res {
                        Ok(_) => {
                            let cloned_vault = vault.clone(); // CLONE
                            return Action::Run(Task::perform(
                                async move { cloned_vault.save().await },
                                Message::SavedVault,
                            ));
                        }
                        Err(err) => {
                            eprintln!("{err}");
                            return Action::None;
                        }
                    }
                }

                Action::None
            }
            Message::UpdateSelectedAlgorithm(algorithm) => {
                #[allow(clippy::collapsible_match)]
                if let State::List { modal, .. } = &mut self.state {
                    if let Modal::AddEdit { entry_config, .. } = modal {
                        entry_config.algorithm = algorithm;
                    }
                }
                Action::None
            }
            Message::ToggleAdvancedConfig => {
                #[allow(clippy::collapsible_match)]
                if let State::List { modal, .. } = &mut self.state {
                    if let Modal::AddEdit { show_advanced, .. } = modal {
                        *show_advanced = !(*show_advanced);
                    }
                }
                Action::None
            }
            Message::UpdateAllTOTP => {
                if let Some(vault) = &self.vault {
                    let mut cloned_vault = vault.clone(); // CLONE
                    return Action::Run(Task::perform(
                        async move { cloned_vault.update_all_totp(Self::REFRESH_RATE).await },
                        Message::UpdatedAllTOTP,
                    ));
                }
                Action::None
            }
            Message::UpdatedAllTOTP(res) => {
                if let Some(vault) = &mut self.vault {
                    match res {
                        Ok(entries) => {
                            let substituted_entries = vault.substitute_entries(entries);
                            match substituted_entries {
                                Ok(_) => {
                                    return Action::None;
                                }
                                Err(err) => {
                                    eprintln!("Error substituting entries: {err}");
                                }
                            }
                        }
                        Err(err) => {
                            eprintln!("Error generating TOTPS: {err}");
                        }
                    }
                }
                Action::None
            }
            Message::UpdateTimeCount => {
                if let State::List {
                    modal: _,
                    time_count,
                } = &mut self.state
                {
                    if time_count > &mut 0 {
                        *time_count -= 1;
                    } else {
                        *time_count = Self::REFRESH_RATE;
                        return self.update(Message::UpdateAllTOTP, now);
                    }
                }
                Action::None
            }
        }
    }

    pub fn subscription(&self, _now: Instant) -> Subscription<Message> {
        let mut subscriptions = vec![];
        match &self.state {
            State::Creation {
                new_password: _,
                new_password_repeat: _,
            } => subscriptions.push(event::listen_with(handle_event)),
            State::Decryption { password: _ } => {
                subscriptions.push(event::listen_with(handle_event))
            }
            State::List {
                modal: current_modal,
                time_count: _,
            } => {
                subscriptions.push(
                    iced::time::every(std::time::Duration::from_secs(1))
                        .map(|_| Message::UpdateTimeCount),
                );

                match &current_modal {
                    Modal::None => {}
                    Modal::AddEdit { .. } => subscriptions.push(event::listen_with(handle_event)),
                    Modal::Config => {}
                }
            }
        }

        Subscription::batch(subscriptions)
    }

    pub fn view(&self, _now: Instant) -> Element<Message> {
        let content = match &self.state {
            State::Creation {
                new_password,
                new_password_repeat,
            } => column![
                text(Self::APP_TITLE)
                    .size(35.)
                    .width(Length::Fill)
                    .align_x(Alignment::Center),
                container(
                    column![
                        text_input("New password", new_password)
                            .secure(true)
                            .on_submit_maybe(maybe_matching_passwords(
                                new_password,
                                new_password_repeat,
                                Message::CreateVault
                            ))
                            .on_input(|s| Message::TextInputted(TextInputs::NewPassword, s)),
                        text_input("Repeat new password", new_password_repeat)
                            .secure(true)
                            .on_submit_maybe(maybe_matching_passwords(
                                new_password,
                                new_password_repeat,
                                Message::CreateVault
                            ))
                            .on_input(|s| Message::TextInputted(TextInputs::NewPasswordRepeat, s)),
                        button("Create")
                            .on_press_maybe(maybe_matching_passwords(
                                new_password,
                                new_password_repeat,
                                Message::CreateVault
                            ))
                            .width(Length::Fill)
                    ]
                    .spacing(5.)
                )
            ],
            State::Decryption { password } => column![
                text(Self::APP_TITLE)
                    .size(35.)
                    .width(Length::Fill)
                    .align_x(Alignment::Center),
                container(
                    column![
                        text_input("Enter Password", password)
                            .secure(true)
                            .on_submit(Message::UnlockVault)
                            .on_input(|s| Message::TextInputted(TextInputs::Password, s)),
                        button("Unlock")
                            .on_press(Message::UnlockVault)
                            .width(Length::Fill)
                    ]
                    .spacing(5.)
                )
            ],
            State::List { modal, time_count } => {
                let header =
                    row![
                        text(format!("{} ({})", Self::APP_TITLE, time_count)).width(Length::Fill),
                        button(icons::get_icon("list-add-symbolic", 21))
                            .style(rounded_primary_button)
                            .on_press(self.determine_modal_button_function(Message::OpenModal(
                                Modal::add_edit(None)
                            ))),
                        button(icons::get_icon("emblem-system-symbolic", 21))
                            .style(rounded_primary_button)
                            .on_press(self.determine_modal_button_function(Message::OpenModal(
                                Modal::config()
                            )))
                    ]
                    .spacing(5.)
                    .width(Length::Fill);

                let content = if let Some(vault) = &self.vault {
                    match modal {
                        Modal::None => {
                            if let Some(entries) = vault.entries() {
                                if entries.is_empty() {
                                    container(text("No entries..."))
                                } else {
                                    let entries_content: Element<Message> = column(
                                        entries
                                            .values()
                                            .map(|e| {
                                                mouse_area(
                                                    container(
                                                        row![
                                                            text(&e.name)
                                                                .shaping(text::Shaping::Advanced)
                                                                .size(20.)
                                                                .width(Length::Fill),
                                                            text(&e.totp).size(20.),
                                                            button(icons::get_icon(
                                                                "edit-symbolic",
                                                                21
                                                            ))
                                                            .style(rounded_primary_button)
                                                            .on_press(Message::OpenModal(
                                                                Modal::add_edit(Some(e.clone()))
                                                            ))
                                                        ]
                                                        .align_y(Alignment::Center)
                                                        .spacing(10.),
                                                    )
                                                    .style(rounded_container)
                                                    .padding(10.),
                                                )
                                                .on_press(Message::SetClipboardContent(
                                                    e.totp.clone(),
                                                ))
                                                .into()
                                            })
                                            .collect::<Vec<Element<Message>>>(),
                                    )
                                    .spacing(10.)
                                    .into();
                                    container(scrollable(column![entries_content, text(format!("{} - Entries", &entries.len())).align_x(Alignment::Center).width(Length::Fill)].spacing(5.)).spacing(5.))
                                }
                            } else {
                                container(text("Error, getting vault entries..."))
                            }
                        }
                        Modal::AddEdit {
                            entry_id,
                            entry_name,
                            entry_secret,
                            entry_config,
                            show_advanced,
                            next_input_clean: _,
                        } => container(custom_modal(self.add_modal_view(
                            entry_id,
                            entry_name,
                            entry_secret,
                            entry_config,
                            show_advanced,
                        ))),
                        Modal::Config => container(custom_modal(self.config_modal_view())),
                    }
                } else {
                    container(text("Error, no vault found..."))
                };

                column![
                    header,
                    container(content)
                        .align_x(Alignment::Center)
                        .width(Length::Fill)
                        .height(Length::Fill)
                ]
                .spacing(10.)
            }
        };

        container(content)
            .center(Length::Fill)
            .padding(Padding::new(10.))
            .into()
    }

    fn add_modal_view(
        &self,
        entry_id: &Option<entry::Id>,
        entry_name: &String,
        entry_secret: &String,
        totp_config: &TOTPConfig,
        show_advanced: &bool,
    ) -> Element<Message> {
        let entry = Entry {
            id: *entry_id,
            name: entry_name.to_string(),
            secret: entry_secret.to_string(),
            totp: String::new(),
            totp_config: totp_config.clone(),
        };

        let header = row![
            text("Add").width(Length::Fill),
            button("Delete")
                .on_press_maybe(entry.id.map(Message::DeleteEntry))
                .style(rounded_danger_button),
            button("Close")
                .style(rounded_primary_button)
                .on_press(Message::OpenModal(Modal::close())),
            button(icons::get_icon("x-office-document-symbolic", 21))
                .style(rounded_primary_button)
                .on_press(Message::ToggleAdvancedConfig)
        ]
        .align_y(Alignment::Center)
        .spacing(5.);

        let can_save: Option<Message> = if entry.is_valid() {
            Some(Message::UpsertEntry(entry))
        } else {
            None
        };

        let algorithm_label = text("Algorithm").size(13.).width(Length::Fill);
        let algorithm_selector = column![
            algorithm_label,
            pick_list(
                TOTPConfig::get_all_algorithms(),
                Some(totp_config.algorithm),
                Message::UpdateSelectedAlgorithm,
            )
            .width(Length::Fill)
        ]
        .spacing(2.);

        let advanced_content = container(
            column![
                algorithm_selector,
                column![
                    text("Digits").size(13.).width(Length::Fill),
                    text_input("Digits", &totp_config.digits.to_string())
                        .on_input(|s| Message::TextInputted(TextInputs::EntryConfigDigits, s))
                        .width(Length::Fill)
                ]
                .spacing(2.),
                column![
                    text("Skew").size(13.).width(Length::Fill),
                    text_input("Skew", &totp_config.skew.to_string())
                        .on_input(|s| Message::TextInputted(TextInputs::EntryConfigSkew, s))
                        .width(Length::Fill)
                ]
                .spacing(2.),
            ]
            .spacing(6.),
        )
        .width(Length::Fill);

        let content = container(
            column![
                column![
                    text("Name").size(13.).width(Length::Fill),
                    text_input("Name", entry_name)
                        .on_input(|s| Message::TextInputted(TextInputs::EntryName, s))
                        .on_submit_maybe(can_save.clone())
                        .width(Length::Fill)
                ]
                .spacing(2.),
                column![
                    text("Secret").size(13.).width(Length::Fill),
                    text_input("Secret", entry_secret)
                        .on_input(|s| Message::TextInputted(TextInputs::EntrySecret, s))
                        .on_submit_maybe(can_save.clone())
                        .width(Length::Fill)
                ]
                .spacing(2.),
                button("Save").width(Length::Fill).on_press_maybe(can_save)
            ]
            .spacing(6.),
        )
        .width(Length::Fill);

        if *show_advanced {
            column![header, advanced_content, content]
                .spacing(10.)
                .into()
        } else {
            column![header, content].spacing(10.).into()
        }
    }

    fn config_modal_view(&self) -> Element<Message> {
        let header = row![
            text("Configuration").width(Length::Fill),
            button("Close")
                .style(rounded_primary_button)
                .on_press(Message::OpenModal(Modal::close()))
        ]
        .align_y(Alignment::Center);

        let content = container(
            column![
            column![
                column![
                    row![
                        text("Export/Import unencrypted vault (Custom Format)").width(Length::Fill),
                        tooltip(
                            button(text("i")).style(rounded_primary_button),
                            container(text("The custom format is only compatible with Clockode")).padding(3.).style(container::primary),
                            tooltip::Position::Top
                        )
                    ],
                    row![
                        button("Export").on_press(Message::OpenExportVaultDialog(ExportImportType::Custom)),
                        button("Import").on_press(Message::OpenImportVaultDialog(ExportImportType::Custom))
                    ]
                    .spacing(5.)
                ]
                .spacing(3.),
                column![
                    row![
                        text("Export/Import unencrypted vault (Standard Backup Format)")
                            .width(Length::Fill),
                        tooltip(
                            button(text("i")).style(rounded_primary_button),
                            container(text("This format is compatible with Aegis, Authenticator(GNOME), FreeOTP+...")).padding(3.).style(container::primary),
                            tooltip::Position::Top
                        )
                    ],
                    row![
                        button("Export").on_press(Message::OpenExportVaultDialog(ExportImportType::Standard)),
                        button("Import").on_press(Message::OpenImportVaultDialog(ExportImportType::Standard))
                    ]
                    .spacing(5.)
                ]
                .spacing(3.),
                column![
                    text("Application theme"),
                    pick_list(
                        Theme::ALL,
                        Some::<Theme>(self.config.theme.clone().into()),
                        |t| {
                            Message::ChangedTheme(ColockodeTheme::try_from(&t).unwrap_or_default())
                        }
                    )
                    .width(Length::Fill)
                ]
                .spacing(3.)
            ]
            .height(Length::Fill)
            .spacing(10.), 
                text(format!("Version: {}", env!("CARGO_PKG_VERSION"))).width(Length::Fill).align_x(Alignment::Center).font(iced::font::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            })]
        )
        .height(Length::Fill);

        column![header, content].spacing(10.).into()
    }

    fn determine_modal_button_function(&self, open: Message) -> Message {
        if let State::List { modal, .. } = &self.state {
            match modal {
                Modal::None => open,
                Modal::AddEdit { .. } => Message::OpenModal(Modal::close()),
                Modal::Config => Message::OpenModal(Modal::close()),
            }
        } else {
            open
        }
    }
}

fn maybe_matching_passwords(
    password: &String,
    repeat_password: &String,
    result_message: Message,
) -> Option<Message> {
    if password.eq(repeat_password) && !password.is_empty() {
        Some(result_message)
    } else {
        None
    }
}

/// Calculates how many seconds remain until the next TOTP refresh
/// based on standard TOTP timing (synchronized to Unix epoch)
///
/// # Arguments
/// * `refresh_rate` - The TOTP refresh interval in seconds (typically 30)
///
/// # Returns
/// The number of seconds until the next refresh occurs
fn get_time_until_next_totp_refresh(refresh_rate: u64) -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let seconds: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Calculate remaining seconds until next window
    // This will be synchronized with other TOTP apps since they
    // all count from the same Unix epoch reference point
    refresh_rate - (seconds % refresh_rate)
}

fn custom_modal(content: Element<Message>) -> Element<Message> {
    float(
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(10.)
            .style(container::secondary),
    )
    .into()
}

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
