// SPDX-License-Identifier: GPL-3.0-only

use core::vault::Vault;

use iced::{
    Element, Font, Subscription, Task, Theme,
    time::Instant,
    widget::{center, text},
};
use screen::{Screen, vault};

mod core;
mod screen;

fn main() -> iced::Result {
    iced::application::timed(
        Clockode::new,
        Clockode::update,
        Clockode::subscription,
        Clockode::view,
    )
    .theme(Clockode::theme)
    .default_font(Font::MONOSPACE)
    .window_size((400., 700.))
    .run()
}

struct Clockode {
    state: State,
    now: Instant,
}

enum State {
    Loading,
    Ready { screen: Screen },
}

#[derive(Debug, Clone)]
enum Message {
    Loaded(Result<Vault, anywho::Error>),

    Vault(vault::Message),
}

impl Clockode {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                state: State::Loading,
                now: Instant::now(),
            },
            Task::perform(async { Vault::load().await }, Message::Loaded),
        )
    }

    fn update(&mut self, message: Message, now: Instant) -> Task<Message> {
        self.now = now;

        match message {
            Message::Loaded(res) => {
                let vault_screen = screen::Vault::new(res);
                self.state = State::Ready {
                    screen: Screen::Vault(vault_screen),
                };

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
                }
            }
        }
    }

    fn view(&self) -> Element<Message> {
        match &self.state {
            State::Loading => center(text("Loading...")).into(),
            State::Ready { screen } => match screen {
                Screen::Vault(vault) => vault.view(self.now).map(Message::Vault),
            },
        }
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
        Theme::CatppuccinMocha
    }
}
