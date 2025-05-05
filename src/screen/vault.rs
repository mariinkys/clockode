use std::collections::HashMap;

use iced::time::Instant;
use iced::widget::{
    button, column, container, float, mouse_area, pick_list, row, text, text_input,
};
use iced::{Alignment, Element, Length, Padding, Subscription, Task};

use crate::core::entry::{self, Algorithm, Entry, TOTPConfig};

pub struct Vault {
    state: State,
    vault: Option<crate::Vault>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TextInputted(TextInputs, String),

    CreateVault,
    CreatedVault(Result<crate::Vault, anywho::Error>),

    UnlockVault,
    UnlockedVault(Result<crate::Vault, anywho::Error>),
    SavedVault(Result<(), anywho::Error>),

    OpenModal(Modal),

    UpsertEntry(Entry),
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

impl Vault {
    const APP_TITLE: &str = "Iced 2FA";
    const REFRESH_RATE: u64 = 30;

    pub fn new(vault: Result<crate::Vault, anywho::Error>) -> Self {
        if let Ok(vault) = vault {
            Self {
                state: State::Decryption {
                    password: String::new(),
                },
                vault: Some(vault),
            }
        } else {
            Self {
                state: State::Creation {
                    new_password: String::new(),
                    new_password_repeat: String::new(),
                },
                vault: None,
            }
        }
    }

    pub fn update(&mut self, message: Message, now: Instant) -> Action {
        match message {
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
                        eprintln!("{}", err);
                    }
                }
                Action::None
            }
            Message::UnlockVault => {
                if let Some(vault) = &self.vault {
                    if let State::Decryption { password, .. } = &mut self.state {
                        let password = std::mem::take(password);
                        Action::Run(Task::perform(
                            crate::Vault::decrypt(password, vault.clone()), //TODO: DO NOT CLONE HERE
                            Message::UnlockedVault,
                        ))
                    } else {
                        Action::None
                    }
                } else {
                    Action::None
                }
            }
            Message::UnlockedVault(res) => {
                match res {
                    Ok(vault) => {
                        self.state = State::List {
                            modal: Modal::None,
                            time_count: Self::REFRESH_RATE,
                        };
                        self.vault = Some(vault);
                        return self.update(Message::UpdateAllTOTP, now);
                    }
                    Err(err) => {
                        eprintln!("{}", err);
                    }
                }
                Action::None
            }
            Message::SavedVault(res) => {
                match res {
                    Ok(_) => {
                        if let State::List { modal, .. } = &mut self.state {
                            *modal = Modal::close();
                        }
                    }
                    Err(err) => {
                        eprintln!("Error saving vault: {}", err);
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
                            let cloned_vault = vault.clone(); // TODO: DO NOT CLONE HERE
                            return Action::Run(Task::perform(
                                async move { cloned_vault.save().await },
                                Message::SavedVault,
                            ));
                        }
                        Err(err) => {
                            eprintln!("{}", err);
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
                    let mut cloned_vault = vault.clone(); // TODO: DO NOT CLONE HERE
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
                                    eprintln!("Error substituting entries: {}", err);
                                }
                            }
                        }
                        Err(err) => {
                            eprintln!("Error generating TOTPS: {}", err);
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

    pub fn subscription(&self, now: Instant) -> Subscription<Message> {
        match &self.state {
            State::Creation {
                new_password: _,
                new_password_repeat: _,
            } => Subscription::none(),
            State::Decryption { password: _ } => Subscription::none(),
            State::List {
                modal: _,
                time_count: _,
            } => iced::time::every(std::time::Duration::from_secs(1))
                .map(|_| Message::UpdateTimeCount),
        }
    }

    pub fn view(&self, now: Instant) -> Element<Message> {
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
                let header = row![
                    text(format!("{} ({})", Self::APP_TITLE, time_count)).width(Length::Fill),
                    button("+").on_press(self.determine_modal_button_function(Message::OpenModal(
                        Modal::add_edit(None)
                    ))),
                    button("C").on_press(
                        self.determine_modal_button_function(Message::OpenModal(Modal::config()))
                    )
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
                                            .iter()
                                            .map(|(_, e)| {
                                                mouse_area(
                                                    container(
                                                        row![
                                                            text(&e.name)
                                                                .size(20.)
                                                                .width(Length::Fill),
                                                            text(&e.totp).size(20.),
                                                            button(text("E").center()).on_press(
                                                                Message::OpenModal(
                                                                    Modal::add_edit(Some(
                                                                        e.clone()
                                                                    ))
                                                                )
                                                            )
                                                        ]
                                                        .align_y(Alignment::Center)
                                                        .spacing(10.),
                                                    )
                                                    .style(container::rounded_box)
                                                    .padding(10.),
                                                )
                                                .into()
                                            })
                                            .collect::<Vec<Element<Message>>>(),
                                    )
                                    .spacing(10.)
                                    .into();
                                    container(column![entries_content])
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
        let header = row![
            text("Add").width(Length::Fill),
            button("Close").on_press(Message::OpenModal(Modal::close())),
            button("A").on_press(Message::ToggleAdvancedConfig)
        ]
        .spacing(5.);

        let entry = Entry {
            id: *entry_id,
            name: entry_name.to_string(),
            secret: entry_secret.to_string(),
            totp: String::new(),
            totp_config: totp_config.clone(),
        };

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
                text("Advanced"),
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
                        .width(Length::Fill)
                ]
                .spacing(2.),
                column![
                    text("Secret").size(13.).width(Length::Fill),
                    text_input("Secret", entry_secret)
                        .on_input(|s| Message::TextInputted(TextInputs::EntrySecret, s))
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
            text("Config").width(Length::Fill),
            button("Close").on_press(Message::OpenModal(Modal::close()))
        ];

        let content = container(text("Testing")).height(Length::Fill);

        column![header, content].into()
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
