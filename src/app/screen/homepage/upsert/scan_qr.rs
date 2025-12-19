// SPDX-License-Identifier: GPL-3.0-only

use iced::{
    Element,
    Length::{self},
    Subscription, Task,
    time::Instant,
    widget::{container, text},
};

use crate::app::widgets::Toast;

pub struct QrScanPage {}

#[derive(Debug, Clone)]
pub enum Message {
    /// Go back a screen
    Back,
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

impl QrScanPage {
    pub fn new() -> (Self, Task<Message>) {
        (Self {}, Task::none())
    }

    pub fn view(&self, _now: Instant) -> iced::Element<'_, Message> {
        let content = qr_scan_view();

        container(content).padding(5.).center(Length::Fill).into()
    }

    pub fn update(&mut self, message: Message, _now: Instant) -> Action {
        match message {
            Message::Back => Action::Back,
        }
    }

    pub fn subscription(&self, _now: Instant) -> Subscription<Message> {
        Subscription::none()
    }
}

fn qr_scan_view<'a>() -> Element<'a, Message> {
    text("Hello").into()
}
