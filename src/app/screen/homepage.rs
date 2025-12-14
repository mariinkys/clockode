// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;

use iced::{
    Alignment, Element,
    Length::{self},
    Subscription, Task,
    time::Instant,
    widget::{Column, button, column, container, row, scrollable, text},
};

use crate::app::{
    core::{ClockodeDatabase, ClockodeEntry},
    widgets::Toast,
};

mod settings;
mod upsert;

pub struct HomePage {
    database: Arc<ClockodeDatabase>,
    state: State,
}

pub enum State {
    Loading,
    Ready { subscreen: SubScreen },
}

pub enum SubScreen {
    Home { entries: Vec<ClockodeEntry> },
    UpsertPage(upsert::UpsertPage),
}

#[derive(Debug, Clone)]
pub enum Message {
    LoadEntries,
    EntriesLoaded(Result<Vec<ClockodeEntry>, anywho::Error>),

    UpsertPage(upsert::Message),
    OpenUpsertPage(Option<ClockodeEntry>),
    EntryUpserted(Result<(), anywho::Error>),
}

pub enum Action {
    /// Does nothing
    None,
    /// Ask parent to run an [`iced::Task`]
    Run(Task<Message>),
    /// Add a new [`Toast`] to show
    AddToast(Toast),
    /// Ask parent to run an [`iced::Task`] and add a [`Toast`] to show
    RunAndToast(Task<Message>, Toast),
}

impl HomePage {
    pub fn new(database: Arc<ClockodeDatabase>) -> (Self, Task<Message>) {
        let db_clone = Arc::clone(&database);

        (
            Self {
                database,
                state: State::Loading,
            },
            Task::perform(
                async move { db_clone.list_entries().await },
                Message::EntriesLoaded,
            ),
        )
    }

    pub fn view(&self, now: Instant) -> iced::Element<'_, Message> {
        let content: Element<Message> = match &self.state {
            State::Loading => text("Loading...").into(),
            State::Ready { subscreen } => match subscreen {
                SubScreen::Home { entries } => {
                    let header = header_view();
                    let content = content_view(entries);

                    container(column![header, content])
                        .padding(5.)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into()
                }
                SubScreen::UpsertPage(upsert_page) => {
                    upsert_page.view(now).map(Message::UpsertPage)
                }
            },
        };

        container(content).center(Length::Fill).into()
    }

    pub fn update(&mut self, message: Message, now: Instant) -> Action {
        match message {
            Message::LoadEntries => {
                self.state = State::Loading;

                let db_clone = Arc::clone(&self.database);
                Action::Run(Task::perform(
                    async move { db_clone.list_entries().await },
                    Message::EntriesLoaded,
                ))
            }
            Message::EntriesLoaded(result) => match result {
                Ok(entries) => {
                    self.state = State::Ready {
                        subscreen: SubScreen::Home { entries },
                    };
                    Action::None
                }
                Err(err) => {
                    eprintln!("{}", err);
                    Action::AddToast(Toast::error_toast(err))
                }
            },

            Message::UpsertPage(message) => {
                let State::Ready { subscreen } = &mut self.state else {
                    return Action::None;
                };

                let SubScreen::UpsertPage(upsert_page) = subscreen else {
                    return Action::None;
                };

                match upsert_page.update(message, now) {
                    upsert::Action::None => Action::None,
                    upsert::Action::Back => self.update(Message::LoadEntries, now),
                    //upsert::Action::Run(task) => Action::Run(task.map(Message::UpsertPage)),
                    upsert::Action::AddToast(toast) => Action::AddToast(toast),
                    upsert::Action::UpdateEntry(clockode_entry) => {
                        let db_clone = Arc::clone(&self.database);
                        Action::Run(Task::perform(
                            async move { db_clone.update_entry(clockode_entry).await },
                            Message::EntryUpserted,
                        ))
                    }
                    upsert::Action::CreateEntry(clockode_entry) => {
                        let db_clone = Arc::clone(&self.database);
                        Action::Run(Task::perform(
                            async move { db_clone.add_entry(clockode_entry).await },
                            Message::EntryUpserted,
                        ))
                    }
                }
            }
            Message::OpenUpsertPage(entry) => {
                let State::Ready { subscreen, .. } = &mut self.state else {
                    return Action::None;
                };

                let (upsert_page, task) = upsert::UpsertPage::new(entry);
                *subscreen = SubScreen::UpsertPage(upsert_page);
                Action::Run(task.map(Message::UpsertPage))
            }
            Message::EntryUpserted(result) => match result {
                Ok(_) => self.update(Message::LoadEntries, now),
                Err(err) => {
                    self.state = State::Loading;
                    let db_clone = Arc::clone(&self.database);
                    Action::RunAndToast(
                        Task::perform(
                            async move { db_clone.list_entries().await },
                            Message::EntriesLoaded,
                        ),
                        Toast::error_toast(err),
                    )
                }
            },
        }
    }

    pub fn subscription(&self, now: Instant) -> Subscription<Message> {
        let State::Ready { subscreen, .. } = &self.state else {
            return Subscription::none();
        };

        match subscreen {
            SubScreen::Home { .. } => Subscription::none(),
            SubScreen::UpsertPage(upsert_page) => {
                upsert_page.subscription(now).map(Message::UpsertPage)
            }
        }
    }
}

/// View of the header of this screen
fn header_view<'a>() -> Element<'a, Message> {
    row![button("Add").on_press(Message::OpenUpsertPage(None))]
        .width(Length::Fill)
        .height(Length::Fixed(30.))
        .into()
}

/// View of the contents of this screen
fn content_view<'a>(entries: &'a [ClockodeEntry]) -> Element<'a, Message> {
    if entries.is_empty() {
        container(
            column![
                text("No TOTP entries found").size(24),
                text("Add your first entry to get started").size(14),
            ]
            .align_x(Alignment::Center)
            .spacing(10),
        )
        .center(Length::Fill)
        .into()
    } else {
        let entries_list = entries.iter().fold(
            Column::new().height(Length::Fill).spacing(12).padding(20),
            |col, entry| {
                let code = entry.totp.generate_current().unwrap_or_default();

                let entry_view = container(
                    row![
                        column![
                            text(&entry.name).size(18),
                            text(format!(
                                "{} digits · {}s",
                                entry.totp.digits, entry.totp.step
                            ))
                            .size(12)
                            .style(|theme: &iced::Theme| {
                                text::Style {
                                    color: Some(theme.palette().text.scale_alpha(0.6)),
                                }
                            }),
                        ]
                        .spacing(4)
                        .width(Length::Fill),
                        column![text(code).size(28).font(iced::Font::MONOSPACE)]
                            .spacing(4)
                            .align_x(iced::Alignment::End),
                        button("Copy").padding(8),
                    ]
                    .spacing(20)
                    .padding(16)
                    .align_y(iced::Alignment::Center),
                )
                .style(|theme: &iced::Theme| container::Style {
                    background: Some(theme.palette().background.into()),
                    border: iced::Border {
                        color: theme.palette().text.scale_alpha(0.1),
                        width: 1.0,
                        radius: 8.0.into(),
                    },
                    ..Default::default()
                });

                col.push(entry_view)
            },
        );

        scrollable(entries_list).height(Length::Fill).into()
    }
}
