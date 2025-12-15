// SPDX-License-Identifier: GPL-3.0-only

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use arboard::Clipboard;
use iced::{
    Alignment, Element,
    Length::{self},
    Subscription, Task,
    time::Instant,
    widget::{Column, button, column, container, row, scrollable, space, text},
};

use crate::{
    app::{
        core::{ClockodeDatabase, ClockodeEntry},
        utils::{get_time_until_next_totp_refresh, style},
        widgets::Toast,
    },
    config::Config,
    icons,
};

mod settings;
mod upsert;

pub struct HomePage {
    config: Arc<Mutex<Config>>,
    clipboard: Option<Clipboard>,
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
    SettingsPage(settings::SettingsPage),
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Attempt to copy some [`String`] to the user clipboard
    CopyToClipboard(String),
    /// Ask to load the [`ClockodeEntry`]s to list on the page
    LoadEntries,
    /// Callback after asking to load [`ClockodeEntry`]s, set's the entries on the state if Ok
    EntriesLoaded(Result<Vec<ClockodeEntry>, anywho::Error>),

    /// Messages of the [`UpsertPage`]
    UpsertPage(upsert::Message),
    /// Ask to open the [`ClockodeEntry`]  [`UpsertPage`]
    OpenUpsertPage(Option<ClockodeEntry>),
    /// Callback after upserting a [`ClockodeEntry`]
    EntryUpserted(Result<(), anywho::Error>),

    /// Messages of the [`SettingsPage`]
    SettingsPage(settings::Message),
    /// Ask to open the [`SettingsPage`]
    OpenSettingsPage,

    /// Makes iced rerun the view to refresh and tick the timers, runs every second on a subscription
    RefreshCodes,
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
    pub fn new(
        database: Arc<ClockodeDatabase>,
        config: Arc<Mutex<Config>>,
    ) -> (Self, Task<Message>) {
        let db_clone = Arc::clone(&database);
        let clipboard = Clipboard::new();
        if let Err(clip_err) = &clipboard {
            eprintln!("{clip_err}");
        };

        (
            Self {
                config,
                clipboard: clipboard.ok(),
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
                SubScreen::SettingsPage(settings_page) => {
                    settings_page.view(now).map(Message::SettingsPage)
                }
            },
        };

        container(content).center(Length::Fill).into()
    }

    pub fn update(&mut self, message: Message, now: Instant) -> Action {
        match message {
            Message::CopyToClipboard(value) => {
                if let Some(clipboard) = &mut self.clipboard {
                    let res = &clipboard.set_text(value);
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
                    upsert::Action::Run(task) => Action::Run(task.map(Message::UpsertPage)),
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
                    upsert::Action::DeleteEntry(uuid) => {
                        let db_clone = Arc::clone(&self.database);
                        Action::Run(Task::perform(
                            async move { db_clone.delete_entry(uuid).await },
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

            Message::SettingsPage(message) => {
                let State::Ready { subscreen } = &mut self.state else {
                    return Action::None;
                };

                let SubScreen::SettingsPage(settings_page) = subscreen else {
                    return Action::None;
                };

                match settings_page.update(message, now) {
                    settings::Action::None => Action::None,
                    settings::Action::Back => self.update(Message::LoadEntries, now),
                    settings::Action::Run(task) => Action::Run(task.map(Message::SettingsPage)),
                    settings::Action::AddToast(toast) => Action::AddToast(toast),
                }
            }
            Message::OpenSettingsPage => {
                let State::Ready { subscreen, .. } = &mut self.state else {
                    return Action::None;
                };

                let (settings_page, task) = settings::SettingsPage::new(Arc::clone(&self.config));
                *subscreen = SubScreen::SettingsPage(settings_page);
                Action::Run(task.map(Message::SettingsPage))
            }

            Message::RefreshCodes => {
                // This forces a re-render every second
                // Since view() calls totp.generate_current(), codes will update automatically
                Action::None
            }
        }
    }

    pub fn subscription(&self, now: Instant) -> Subscription<Message> {
        let State::Ready { subscreen, .. } = &self.state else {
            return Subscription::none();
        };

        match subscreen {
            SubScreen::Home { entries } => {
                if entries.is_empty() {
                    Subscription::none()
                } else {
                    iced::time::every(Duration::from_secs(1)).map(|_| Message::RefreshCodes)
                }
            }
            SubScreen::UpsertPage(upsert_page) => {
                upsert_page.subscription(now).map(Message::UpsertPage)
            }
            SubScreen::SettingsPage(settings_page) => {
                settings_page.subscription(now).map(Message::SettingsPage)
            }
        }
    }
}

/// View of the header of this screen
fn header_view<'a>() -> Element<'a, Message> {
    row![
        // Title section
        column![
            text("Clockode").size(style::font_size::TITLE),
            text("Two-Factor Authentication")
                .size(style::font_size::SMALL)
                .style(style::muted_text),
        ]
        .spacing(style::spacing::TINY),
        space().width(Length::Fill),
        // Action buttons
        row![
            button(icons::get_icon("list-add-symbolic", 21))
                .on_press(Message::OpenUpsertPage(None))
                .padding(8)
                .style(style::primary_button),
            button(icons::get_icon("emblem-system-symbolic", 21))
                .on_press(Message::OpenSettingsPage)
                .padding(8)
                .style(style::secondary_button),
        ]
        .spacing(style::spacing::SMALL)
    ]
    .spacing(style::spacing::LARGE)
    .padding(20)
    .align_y(iced::Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// View of the contents of this screen
fn content_view<'a>(entries: &'a [ClockodeEntry]) -> Element<'a, Message> {
    if entries.is_empty() {
        container(
            column![
                text("No TOTP entries found").size(style::font_size::TITLE),
                text("Add your first entry to get started").size(style::font_size::BODY),
            ]
            .align_x(Alignment::Center)
            .spacing(style::spacing::MEDIUM),
        )
        .center(Length::Fill)
        .into()
    } else {
        let entries_list = entries.iter().fold(
            Column::new()
                .height(Length::Fill)
                .spacing(style::spacing::MEDIUM)
                .padding(20),
            |col, entry| {
                let code = entry.totp.generate_current().unwrap_or_default();
                let time_remaining = get_time_until_next_totp_refresh(entry.totp.step);

                let entry_view = container(
                    row![
                        column![
                            text(&entry.name).size(style::font_size::LARGE),
                            text(format!(
                                "{} digits · {}s",
                                entry.totp.digits, time_remaining
                            ))
                            .size(style::font_size::SMALL)
                            .style(style::muted_text),
                        ]
                        .spacing(style::spacing::TINY)
                        .width(Length::Fill),
                        column![
                            text(code.clone())
                                .size(style::font_size::HERO)
                                .font(iced::Font::MONOSPACE)
                        ]
                        .spacing(style::spacing::TINY)
                        .align_x(iced::Alignment::End),
                        button(icons::get_icon("edit-copy-symbolic", 21))
                            .on_press(Message::CopyToClipboard(code))
                            .padding(8)
                            .style(style::primary_button),
                        button(icons::get_icon("edit-symbolic", 21))
                            .on_press(Message::OpenUpsertPage(Some(entry.clone())))
                            .padding(8)
                            .style(style::secondary_button),
                    ]
                    .spacing(style::spacing::SMALL)
                    .padding(16)
                    .align_y(iced::Alignment::Center),
                )
                .style(style::entry_card);

                col.push(entry_view)
            },
        );

        scrollable(entries_list).height(Length::Fill).into()
    }
}
