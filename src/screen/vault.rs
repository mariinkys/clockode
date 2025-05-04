use iced::time::Instant;
use iced::widget::{button, column, container, float, row, text, text_input};
use iced::{Alignment, Element, Length, Padding, Task};

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

    OpenModal(Modal),
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
}

#[derive(Debug, Clone)]
pub enum Modal {
    None,
    Add { entry_name: String },
    Config,
}

impl Modal {
    pub fn close() -> Modal {
        Modal::None
    }

    pub fn add() -> Modal {
        Modal::Add {
            entry_name: String::new(),
        }
    }

    pub fn config() -> Modal {
        Modal::Config
    }
}

impl Vault {
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
            Message::TextInputted(text_inputs, value) => match text_inputs {
                TextInputs::NewPassword => {
                    if let State::Creation { new_password, .. } = &mut self.state {
                        *new_password = value;
                    }

                    Action::None
                }
                TextInputs::NewPasswordRepeat => {
                    if let State::Creation {
                        new_password_repeat,
                        ..
                    } = &mut self.state
                    {
                        *new_password_repeat = value;
                    }

                    Action::None
                }
                TextInputs::Password => {
                    if let State::Decryption { password, .. } = &mut self.state {
                        *password = value;
                    }

                    Action::None
                }
            },
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
                            crate::Vault::decrypt(password, vault.clone()), //TODO: DO NOT Clone here
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
                        self.state = State::List { modal: Modal::None };
                        self.vault = Some(vault);
                    }
                    Err(err) => {
                        eprintln!("{}", err);
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
        }
    }

    pub fn view(&self, now: Instant) -> Element<Message> {
        let content = match &self.state {
            State::Creation {
                new_password,
                new_password_repeat,
            } => column![
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
            .spacing(5.),
            State::Decryption { password } => column![
                text_input("Enter Password", password)
                    .secure(true)
                    .on_submit(Message::UnlockVault)
                    .on_input(|s| Message::TextInputted(TextInputs::Password, s)),
                button("Unlock")
                    .on_press(Message::UnlockVault)
                    .width(Length::Fill)
            ]
            .spacing(5.),
            State::List { modal } => {
                let header = row![
                    text("Iced 2FA").width(Length::Fill),
                    button("+").on_press(
                        self.determine_modal_button_function(Message::OpenModal(Modal::add()))
                    ),
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
                                    container(text("Entries"))
                                }
                            } else {
                                container(text("Error, getting vault entries..."))
                            }
                        }
                        Modal::Add { entry_name } => container(custom_modal(self.add_modal_view())),
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

    fn add_modal_view(&self) -> Element<Message> {
        let header = row![
            text("Add").width(Length::Fill),
            button("Close").on_press(Message::OpenModal(Modal::close()))
        ];

        let content = container(text("Testing")).height(Length::Fill);

        column![header, content].into()
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
                Modal::Add { entry_name: _ } => Message::OpenModal(Modal::close()),
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
