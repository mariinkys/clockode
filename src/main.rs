// SPDX-License-Identifier: GPL-3.0-only
#![windows_subsystem = "windows"]

use core::vault::Vault;

use config::Config;
use iced::{
    Element, Font, Subscription, Task, Theme,
    time::Instant,
    widget::{center, text},
};
use screen::{Screen, vault};
use widgets::toast::{self, Toast};

mod config;
mod core;
mod icons;
mod screen;
mod style;
mod widgets;

const APP_ID: &str = "dev.mariinkys.Clockode";

fn main() -> iced::Result {
    // Init the icon cache
    icons::ICON_CACHE.get_or_init(|| std::sync::Mutex::new(icons::IconCache::new()));

    let app_icon = iced::window::icon::from_file_data(
        include_bytes!("../resources/icons/hicolor/scalable/apps/icon.svg"),
        None,
    );

    let platform_settings = {
        #[cfg(target_os = "linux")]
        {
            iced::window::settings::PlatformSpecific {
                application_id: String::from(APP_ID),
                ..Default::default()
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            Default::default()
        }
    };

    iced::application::timed(
        Clockode::new,
        Clockode::update,
        Clockode::subscription,
        Clockode::view,
    )
    .theme(Clockode::theme)
    .default_font(Font::MONOSPACE)
    .window_size((400., 700.))
    .window(iced::window::Settings {
        size: iced::Size {
            width: 400.,
            height: 700.,
        },
        min_size: Some(iced::Size {
            width: 300.,
            height: 400.,
        }),
        icon: app_icon.ok(),
        platform_specific: platform_settings,
        ..Default::default()
    })
    .run()
}

struct Clockode {
    toasts: Vec<Toast>,
    config: Config,
    state: State,
    now: Instant,
}

enum State {
    Loading,
    Ready { screen: Screen },
}

#[derive(Debug, Clone)]
enum Message {
    VaultLoaded(Result<Vault, anywho::Error>),
    ConfigLoaded(Result<Config, anywho::Error>),
    ConfigSaved(Result<(), anywho::Error>),

    Vault(vault::Message),

    AddToast(Toast),
    CloseToast(usize),
}

impl Clockode {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                toasts: Vec::new(),
                config: Config::default(),
                state: State::Loading,
                now: Instant::now(),
            },
            Task::batch([
                Task::perform(Vault::load(), Message::VaultLoaded),
                Task::perform(Config::load(APP_ID), Message::ConfigLoaded),
            ]),
        )
    }

    fn update(&mut self, message: Message, now: Instant) -> Task<Message> {
        self.now = now;

        match message {
            Message::VaultLoaded(res) => {
                let vault_screen = screen::Vault::new(res, self.config.clone());
                self.state = State::Ready {
                    screen: Screen::Vault(vault_screen),
                };

                Task::none()
            }
            Message::ConfigLoaded(res) => {
                match res {
                    Ok(config) => {
                        self.config = config;

                        // If the vault has already loaded we update the config
                        let State::Ready { screen, .. } = &mut self.state else {
                            return Task::none();
                        };
                        let Screen::Vault(vault) = screen;
                        vault.set_config(self.config.clone());
                    }
                    Err(err) => {
                        eprintln!("Error loading config: {err}");
                    }
                }
                Task::none()
            }
            Message::ConfigSaved(res) => {
                if let Err(err) = res {
                    eprintln!("{err}");
                }
                Task::none()
            }
            Message::Vault(message) => {
                let State::Ready { screen, .. } = &mut self.state else {
                    return Task::none();
                };
                let Screen::Vault(vault) = screen;

                match vault.update(message, self.now) {
                    vault::Action::None => Task::none(),
                    vault::Action::Run(task) => task.map(Message::Vault),
                    vault::Action::AddToast(toast) => self.update(Message::AddToast(toast), now),
                    vault::Action::ChangedTheme(colockode_theme) => {
                        self.config.theme = colockode_theme;
                        let new_config = self.config.clone();
                        Task::perform(new_config.save(APP_ID), Message::ConfigSaved)
                    }
                }
            }
            Message::AddToast(toast) => {
                self.toasts.push(toast);
                Task::none()
            }
            Message::CloseToast(index) => {
                self.toasts.remove(index);
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let content = match &self.state {
            State::Loading => center(text("Loading...")).into(),
            State::Ready { screen } => match screen {
                Screen::Vault(vault) => vault.view(self.now).map(Message::Vault),
            },
        };

        toast::Manager::new(content, &self.toasts, Message::CloseToast).into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let State::Ready { screen, .. } = &self.state else {
            return Subscription::none();
        };

        match screen {
            Screen::Vault(vault) => vault.subscription(self.now).map(Message::Vault),
        }
    }

    fn theme(&self) -> Theme {
        self.config.theme.clone().into()
    }
}
