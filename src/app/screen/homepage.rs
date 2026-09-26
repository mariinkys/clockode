// SPDX-License-Identifier: GPL-3.0-only

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use iced::{
    Alignment, Element, Event, Length::{self}, Subscription, Task, event, keyboard, time::Instant, widget::{Column, button, column, container, operation, row, scrollable, space, stack, text, text_input},
};
use tracing::{error, info};

use crate::{
    app::{
        core::{ClockodeDatabase, ClockodeEntry}, utils::{clipboard::{self, AppClipboard}, get_time_until_next_totp_refresh, style, watch_database}, widgets::{Toast, dot},
    }, config::Config, icons,
};

mod settings;
mod upsert;

const SEARCH_INPUT: &str = "home-search";

pub struct HomePage {
    config: Arc<Mutex<Config>>,
    clipboard: AppClipboard,
    database: Arc<ClockodeDatabase>,
    state: State,
    search: Box<Option<String>>,
    /// Whether the search input currently has keyboard focus
    search_focused: bool,
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
    /// Callback when the clipboard is ready to be used
    ClipboardReady(Option<usize>),
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
    /// The database changed (watcher)
    DatabaseChangedOnDisk,

    /// Expand the search icon into a text input
    OpenSearch,
    /// The search query changed
    SearchChanged(String),
    /// Clear the search and collapse it back to an icon
    CloseSearch,
    /// Escape was pressed while the search is open
    SearchEscape,
    /// Something that may change focus happened (click, Tab), re-check it
    CheckSearchFocus,
    /// Result of checking whether the search input is focused
    SearchFocusChanged(bool),
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

        (
            Self {
                config,
                clipboard: AppClipboard::Pending,
                database,
                state: State::Loading,
                search: Box::from(None),
                search_focused: false,
            },
            Task::batch([
                Task::perform(
                    async move { db_clone.list_entries().await },
                    Message::EntriesLoaded,
                ),
                clipboard::resolve_display().map(Message::ClipboardReady),
            ]),
        )
    }

    pub fn view(&self, now: Instant) -> iced::Element<'_, Message> {
        let content: Element<Message> = match &self.state {
            State::Loading => text("Loading...").into(),
            State::Ready { subscreen } => match subscreen {
                SubScreen::Home { entries } => {
                    let search = self.search.as_deref();
                    let header = header_view(entries.len(), search);
                    let content = content_view(entries, search);

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
            Message::ClipboardReady(display) => {
                self.clipboard.init(display);
                Action::None
            }
            Message::CopyToClipboard(value) => match self.clipboard.set_text(value) {
                Ok(()) => Action::AddToast(Toast::success_toast("Copied to clipboard")),
                Err(err) => {
                    error!("{err}");
                    Action::AddToast(Toast::error_toast(err))
                }
            },
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
                    error!("{err}");
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
                    error!("{err}");
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
                    settings::Action::ImportContent(path_buf) => {
                        let db_clone = Arc::clone(&self.database);
                        Action::Run(Task::perform(
                            async move { db_clone.import_content(path_buf).await },
                            Message::EntryUpserted,
                        ))
                    }
                    settings::Action::ExportContent(path_buf) => {
                        let db_clone = Arc::clone(&self.database);
                        Action::Run(Task::perform(
                            async move { db_clone.export_content(path_buf).await },
                            Message::EntryUpserted,
                        ))
                    }
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

            Message::DatabaseChangedOnDisk => {
                info!("Database Changed");

                if !self.database.has_changed_on_disk() {
                    info!("Ignoring filesystem event: no external change");
                    return Action::None;
                }

                let State::Ready { subscreen, .. } = &mut self.state else {
                    return Action::None;
                };

                let SubScreen::Home { entries: _ } = subscreen else {
                    return Action::None;
                };

                self.update(Message::LoadEntries, now)
            }

            Message::OpenSearch => {
                self.search = Box::from(Some(String::new()));
                self.search_focused = true;
                Action::Run(operation::focus(SEARCH_INPUT))
            }
            Message::SearchChanged(query) => {
                if let Some(search) = &mut *self.search {
                    *search = query;
                    self.search_focused = true;
                }
                Action::None
            }
            Message::CloseSearch => {
                self.search = Box::from(None);
                self.search_focused = false;
                Action::None
            }
            Message::SearchEscape => {
                if self.search_focused {
                    self.update(Message::CloseSearch, now)
                } else {
                    Action::None
                }
            }
            Message::CheckSearchFocus => {
                Action::Run(operation::is_focused(SEARCH_INPUT).map(Message::SearchFocusChanged))
            }
            Message::SearchFocusChanged(focused) => {
                self.search_focused = focused;
                Action::None
            }
        }
    }

    pub fn subscription(&self, now: Instant) -> Subscription<Message> {
        let watcher = watch_database((*self.database.path()).clone())
            .map(|_| Message::DatabaseChangedOnDisk);

        let search = if self.search.is_some() {
            event::listen_with(|event, _status, _window| match event {
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                }) => Some(Message::SearchEscape),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Tab),
                    ..
                })
                | Event::Mouse(iced::mouse::Event::ButtonPressed(_)) => Some(Message::CheckSearchFocus),
                _ => None,
            })
        } else {
            Subscription::none()
        };

        let screen_subscription = match &self.state {
            State::Loading => Subscription::none(),
            State::Ready { subscreen } => match subscreen {
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
            },
        };

        Subscription::batch([screen_subscription, watcher, search])
    }
}

/// View of the header of this screen
fn header_view<'a>(entry_count: usize, search: Option<&'a str>) -> Element<'a, Message> {
    row![
        // Title section
        column![
            text("Clockode").size(style::font_size::TITLE),
            text("Two-Factor Authentication")
                .size(style::font_size::SMALL)
                .style(style::muted_text),
            text(format!(
                "{} {}",
                entry_count,
                if entry_count == 1 { "Entry" } else { "Entries" }
            ))
            .size(style::font_size::SMALL)
            .style(style::muted_text)
        ]
        .spacing(style::spacing::TINY),
        space().width(Length::Fill),
        // Action buttons
        row![
            search_view(search),
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
    .padding(10)
    .align_y(iced::Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// View of the contents of this screen
fn content_view<'a>(entries: &'a [ClockodeEntry], search: Option<&'a str>) -> Element<'a, Message> {
    let query = search
            .map(str::trim)
            .filter(|query| !query.is_empty())
            .map(str::to_lowercase);

    let filtered: Vec<&'a ClockodeEntry> = entries
        .iter()
        .filter(|entry| {
            query
                .as_ref()
                .is_none_or(|query| entry.name.to_lowercase().contains(query))
        })
        .collect();

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
    } else if filtered.is_empty() {
            container(
                text("No entries match your search")
                    .size(style::font_size::BODY)
                    .style(style::muted_text),
            )
            .center(Length::Fill)
            .into()
    } else {
        let entries_list = entries.iter().fold(
            Column::new()
                .height(Length::Fill)
                .spacing(style::spacing::MEDIUM)
                .padding(10),
            |col, entry| {
                let code = entry.totp.generate_current().to_string();
                let time_remaining = get_time_until_next_totp_refresh(entry.totp.step());

                let entry_view = container(
                    row![
                        column![
                            text(&entry.name)
                                .wrapping(text::Wrapping::Glyph)
                                .size(style::font_size::LARGE),
                            row![
                                text(format!(
                                    "{} digits · {}s",
                                    entry.totp.digits(), time_remaining
                                ))
                                .size(style::font_size::SMALL)
                                .style(style::muted_text),
                                dot(time_remaining)
                            ]
                            .align_y(Alignment::Start)
                            .spacing(style::spacing::SMALL),
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

/// Search icon that expands into a text input with an inline close button
fn search_view<'a>(search: Option<&'a str>) -> Element<'a, Message> {
    const CLOSE_ICON_SIZE: u16 = 16;
    const CLOSE_PADDING: f32 = 4.0;

    match search {
        None => button(icons::get_icon("system-search-symbolic", 21).style(style::icon))
            .on_press(Message::OpenSearch)
            .padding(8)
            .style(style::transparent_button)
            .into(),
        Some(query) => {
            let input = text_input("Search entries...", query)
                .id(SEARCH_INPUT)
                .on_input(Message::SearchChanged)
                // Extra right padding so typed text never runs under the close button
                .padding(iced::padding::all(8).right(
                    CLOSE_ICON_SIZE as f32 + CLOSE_PADDING * 2.0 + style::spacing::TINY * 2.0,
                ))
                .width(Length::Fixed(200.0))
                .style(style::text_input_style);

            let close = button(
                icons::get_icon("window-close-symbolic", CLOSE_ICON_SIZE).style(style::icon),
            )
            .on_press(Message::CloseSearch)
            .padding(CLOSE_PADDING)
            .style(style::transparent_button);

            stack![
                input,
                container(close)
                    .align_right(Length::Fill)
                    .center_y(Length::Fill)
                    .padding(iced::padding::right(style::spacing::TINY)),
            ]
            .into()
        }
    }
}
