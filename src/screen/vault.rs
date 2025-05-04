use iced::time::Instant;
use iced::widget::{button, column, container, row, text, text_input};
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
}

pub enum State {
    Creation {
        new_password: String,
        new_password_repeat: String,
    },
    Decryption {
        password: String,
    },
    List,
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
            Message::CreatedVault(res) => match res {
                Ok(vault) => {
                    self.state = State::Decryption {
                        password: String::new(),
                    };
                    self.vault = Some(vault);

                    Action::None
                }
                Err(err) => {
                    eprintln!("{}", err);
                    Action::None
                }
            },
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
            Message::UnlockedVault(res) => match res {
                Ok(vault) => {
                    self.state = State::List;
                    self.vault = Some(vault);

                    Action::None
                }
                Err(err) => {
                    eprintln!("{}", err);
                    Action::None
                }
            },
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
            State::List => {
                let header = row![
                    text("Iced 2FA").width(Length::Fill),
                    button("+"),
                    button("C")
                ]
                .spacing(5.)
                .width(Length::Fill);

                let content = if let Some(vault) = &self.vault {
                    if let Some(entries) = vault.entries() {
                        if entries.is_empty() {
                            text("No entries...")
                        } else {
                            text("Entries")
                        }
                    } else {
                        text("Error, getting vault entries...")
                    }
                } else {
                    text("Error, no vault found...")
                };

                column![
                    header,
                    container(content)
                        .align_x(Alignment::Center)
                        .width(Length::Fill)
                        .height(Length::Fill)
                ]
            }
        };

        container(content)
            .center(Length::Fill)
            .padding(Padding::new(10.))
            .into()
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
