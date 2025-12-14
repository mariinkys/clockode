// SPDX-License-Identifier: GPL-3.0-only

use iced::{
    Alignment, Element,
    Length::{self},
    Subscription, Task,
    time::Instant,
    widget::{Column, button, column, container, row, scrollable, text},
};

use crate::app::{core::ClockodeEntry, widgets::Toast};

pub struct UpsertPage {
    entry: Option<ClockodeEntry>,
}

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
    /// Ask parent to run an [`iced::Task`]
    Run(Task<Message>),
    /// Add a new [`Toast`] to show
    AddToast(Toast),
}

impl UpsertPage {
    pub fn new(entry: Option<ClockodeEntry>) -> (Self, Task<Message>) {
        (Self { entry }, Task::none())
    }

    pub fn view(&self, _now: Instant) -> iced::Element<'_, Message> {
        let header = header_view();
        let content: Element<Message> = match &self.entry {
            Some(entry) => update_entry_view(entry),
            None => new_entry_view(),
        };

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
        }
    }

    pub fn subscription(&self, _now: Instant) -> Subscription<Message> {
        Subscription::none()
    }
}

/// View of the header of this screen
fn header_view<'a>() -> Element<'a, Message> {
    row![button("Back").on_press(Message::Back)]
        .width(Length::Fill)
        .height(Length::Fixed(30.))
        .into()
}

fn update_entry_view<'a>(entry: &ClockodeEntry) -> Element<'a, Message> {
    text("Update Entry View").into()
}

fn new_entry_view<'a>() -> Element<'a, Message> {
    text("Update Entry View").into()
}
