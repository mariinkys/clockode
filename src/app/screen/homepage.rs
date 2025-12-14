// SPDX-License-Identifier: GPL-3.0-only

use iced::{
    Length::{self},
    Subscription, Task,
    time::Instant,
    widget::{container, text},
};
use keepass::Database;

pub struct HomePage {
    database: Box<Database>,
}

#[derive(Debug, Clone)]
pub enum Message {}

pub enum Action {
    /// Does nothing
    None,
    /// Ask parent to run an [`iced::Task`]
    Run(Task<Message>),
}

impl HomePage {
    pub fn new(database: Box<Database>) -> (Self, Task<Message>) {
        (Self { database }, Task::none())
    }

    pub fn view(&self, _now: Instant) -> iced::Element<'_, Message> {
        let content = text("Hello");

        container(content).center(Length::Fill).into()
    }

    pub fn update(&mut self, message: Message, _now: Instant) -> Action {
        match message {}
    }

    pub fn subscription(&self, _now: Instant) -> Subscription<Message> {
        Subscription::none()
    }
}
