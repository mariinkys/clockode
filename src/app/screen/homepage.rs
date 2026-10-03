// SPDX-License-Identifier: GPL-3.0-only

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use iced::{
    Alignment, Element, Event,
    Length::{self},
    Rectangle, Subscription, Task, Widget, event, keyboard,
    time::Instant,
    widget::{
        Column, button, column, container, operation, row, scrollable, stack, text, text_input,
    },
};
use tracing::{error, info};

use crate::{
    app::{
        core::{ClockodeDatabase, ClockodeEntry},
        utils::{
            clipboard::{self, AppClipboard},
            get_time_until_next_totp_refresh, style, watch_database,
        },
        widgets::{Toast, dot},
    },
    config::Config,
    icons,
};

mod settings;
mod upsert;

const SEARCH_INPUT: &str = "home-search";
const ENTRIES_SCROLLABLE: &str = "home-entries";
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(150);

pub struct HomePage {
    config: Arc<Mutex<Config>>,
    clipboard: AppClipboard,
    database: Arc<ClockodeDatabase>,
    state: State,
}

pub enum State {
    Loading,
    Ready { subscreen: SubScreen },
}

pub enum SubScreen {
    Home(HomeState),
    UpsertPage(upsert::UpsertPage),
    SettingsPage(settings::SettingsPage),
}

/// State of the home subscreen
pub struct HomeState {
    entries: Vec<ClockodeEntry>,
    /// `None` = collapsed search icon, `Some(query)` = what is typed in the search box
    search: Option<String>,
    /// Whether the search input currently has keyboard focus
    search_focused: bool,
    /// Index (in the filtered list) of the entry highlighted with Tab
    selected_entry: Option<usize>,
    /// The query the entry list is currently filtered by (lags `search` by the debounce)
    applied_search: Option<String>,
    /// Bumped on every keystroke so stale debounce timers can be ignored
    search_generation: u64,
}

impl HomeState {
    fn new(entries: Vec<ClockodeEntry>) -> Self {
        Self {
            entries,
            search: None,
            search_focused: false,
            selected_entry: None,
            applied_search: None,
            search_generation: 0,
        }
    }

    /// Entries matching the applied search query
    fn filtered(&self) -> Vec<&ClockodeEntry> {
        filter_entries(&self.entries, self.applied_search.as_deref())
    }

    /// The entry currently highlighted with Tab, if any
    fn highlighted_entry(&self) -> Option<&ClockodeEntry> {
        let index = self.selected_entry?;
        self.filtered().get(index).copied()
    }

    /// Clears the search and collapses it back to an icon
    fn close_search(&mut self) {
        self.search = None;
        self.applied_search = None;
        // Invalidate any debounce timer still pending
        self.search_generation = self.search_generation.wrapping_add(1);
        self.search_focused = false;
        self.selected_entry = None;
    }

    /// Moves the Tab highlight forwards or backwards (wrapping) and scrolls it into view
    fn move_selection(&mut self, forward: bool) -> Task<Message> {
        let count = self.filtered().len();

        if count == 0 {
            self.selected_entry = None;
            return Task::none();
        }

        let index = match (self.selected_entry, forward) {
            (None, true) => 0,
            (None, false) => count - 1,
            (Some(index), true) => (index + 1) % count,
            (Some(index), false) => (index + count - 1) % count,
        };

        self.selected_entry = Some(index);

        let scroll = scroll_to_entry(index);

        // Highlighting an entry takes over from the search input, so keys like
        // E and Enter act on the entry instead of being typed
        if self.search_focused {
            self.search_focused = false;
            Task::batch([scroll, unfocus_search()])
        } else {
            scroll
        }
    }
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

    /// Expand the search icon into a text input (or refocus it if already open)
    OpenSearch,
    /// The search query changed
    SearchChanged(String),
    /// Debounce timer fired; apply the query if nothing was typed since
    ApplySearch(u64),
    /// Clear the search and collapse it back to an icon
    CloseSearch,
    /// Escape was pressed on the home screen
    SearchEscape,
    /// Something that may change focus happened (click), re-check it
    CheckSearchFocus,
    /// Result of checking whether the search input is focused
    SearchFocusChanged(bool),

    /// Tab: highlight the next entry
    SelectNextEntry,
    /// Shift+Tab: highlight the previous entry
    SelectPreviousEntry,
    /// Enter / C: copy the code of the highlighted entry
    CopySelectedEntry,
    /// E: open the edit page for the highlighted entry
    EditSelectedEntry,
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

    pub fn view(&self, now: Instant) -> impl Widget<Message> {
        let content: Element<Message> = match &self.state {
            State::Loading => text("Loading...").boxed(),
            State::Ready { subscreen } => match subscreen {
                SubScreen::Home(home) => {
                    let header = header_view(home.entries.len(), home.search.as_deref());
                    let content = content_view(
                        &home.entries,
                        home.applied_search.as_deref(),
                        home.selected_entry,
                    );

                    container(column![header, content])
                        .padding(5.)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .boxed()
                }
                SubScreen::UpsertPage(upsert_page) => {
                    upsert_page.view(now).map(Message::UpsertPage).boxed()
                }
                SubScreen::SettingsPage(settings_page) => {
                    settings_page.view(now).map(Message::SettingsPage).boxed()
                }
            },
        };

        container(content).center(Length::Fill)
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
                        subscreen: SubScreen::Home(HomeState::new(entries)),
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

                if self.home().is_none() {
                    return Action::None;
                }

                self.update(Message::LoadEntries, now)
            }

            Message::OpenSearch => {
                let Some(home) = self.home_mut() else {
                    return Action::None;
                };

                if home.search.is_none() {
                    home.search = Some(String::new());
                }

                home.search_focused = true;
                home.selected_entry = None;
                Action::Run(operation::focus(SEARCH_INPUT))
            }
            Message::SearchChanged(query) => {
                let Some(home) = self.home_mut() else {
                    return Action::None;
                };

                let Some(search) = &mut home.search else {
                    return Action::None;
                };

                *search = query;
                let is_empty = search.trim().is_empty();

                home.search_focused = true;
                home.selected_entry = None;
                home.search_generation = home.search_generation.wrapping_add(1);

                if is_empty {
                    home.applied_search = None;
                    return Action::None;
                }

                let generation = home.search_generation;

                Action::Run(Task::perform(
                    async move {
                        smol::Timer::after(SEARCH_DEBOUNCE).await;
                        generation
                    },
                    Message::ApplySearch,
                ))
            }
            Message::ApplySearch(generation) => {
                let Some(home) = self.home_mut() else {
                    return Action::None;
                };

                // A newer keystroke arrived while this timer was waiting
                if generation != home.search_generation {
                    return Action::None;
                }

                home.applied_search = home.search.clone();
                home.selected_entry = None;
                Action::None
            }
            Message::CloseSearch => {
                if let Some(home) = self.home_mut() {
                    home.close_search();
                }
                Action::None
            }
            Message::SearchEscape => {
                if let Some(home) = self.home_mut() {
                    if home.search_focused {
                        home.close_search();
                    } else {
                        home.selected_entry = None;
                    }
                }
                Action::None
            }
            Message::CheckSearchFocus => {
                if self.home().is_none_or(|home| home.search.is_none()) {
                    return Action::None;
                }

                Action::Run(operation::is_focused(SEARCH_INPUT).map(Message::SearchFocusChanged))
            }
            Message::SearchFocusChanged(focused) => {
                if let Some(home) = self.home_mut() {
                    home.search_focused = focused;

                    // Clicking into the search takes over from the entry highlight
                    if focused {
                        home.selected_entry = None;
                    }
                }
                Action::None
            }

            Message::SelectNextEntry => self
                .home_mut()
                .map_or(Action::None, |home| Action::Run(home.move_selection(true))),
            Message::SelectPreviousEntry => self
                .home_mut()
                .map_or(Action::None, |home| Action::Run(home.move_selection(false))),
            Message::CopySelectedEntry => {
                let Some(code) = self
                    .home()
                    .and_then(HomeState::highlighted_entry)
                    .map(|entry| entry.totp.generate_current().to_string())
                else {
                    return Action::None;
                };

                self.update(Message::CopyToClipboard(code), now)
            }
            Message::EditSelectedEntry => {
                let Some(entry) = self.home().and_then(HomeState::highlighted_entry).cloned()
                else {
                    return Action::None;
                };

                self.update(Message::OpenUpsertPage(Some(entry)), now)
            }
        }
    }

    pub fn subscription(&self, now: Instant) -> Subscription<Message> {
        let watcher =
            watch_database((*self.database.path()).clone()).map(|_| Message::DatabaseChangedOnDisk);

        let keys = match &self.state {
            State::Ready {
                subscreen: SubScreen::Home(_),
            } => event::listen_with(|event, status, _window| match event {
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Tab),
                    modifiers,
                    ..
                }) => Some(if modifiers.shift() {
                    Message::SelectPreviousEntry
                } else {
                    Message::SelectNextEntry
                }),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                }) => Some(Message::SearchEscape),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Enter),
                    ..
                }) if status == event::Status::Ignored => Some(Message::CopySelectedEntry),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(c),
                    modifiers,
                    ..
                }) if status == event::Status::Ignored && modifiers.is_empty() => {
                    match c.as_str().to_lowercase().as_str() {
                        "c" => Some(Message::CopySelectedEntry),
                        "e" => Some(Message::EditSelectedEntry),
                        "s" => Some(Message::OpenSearch),
                        _ => None,
                    }
                }
                Event::Mouse(iced::mouse::Event::ButtonPressed(_)) => {
                    Some(Message::CheckSearchFocus)
                }
                _ => None,
            }),
            _ => Subscription::none(),
        };

        let screen_subscription = match &self.state {
            State::Loading => Subscription::none(),
            State::Ready { subscreen } => match subscreen {
                SubScreen::Home(home) => {
                    if home.entries.is_empty() {
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

        Subscription::batch([screen_subscription, watcher, keys])
    }

    /// The home subscreen state, if it's the one currently shown
    fn home(&self) -> Option<&HomeState> {
        match &self.state {
            State::Ready {
                subscreen: SubScreen::Home(home),
            } => Some(home),
            _ => None,
        }
    }

    /// The home subscreen state (mutable), if it's the one currently shown
    fn home_mut(&mut self) -> Option<&mut HomeState> {
        match &mut self.state {
            State::Ready {
                subscreen: SubScreen::Home(home),
            } => Some(home),
            _ => None,
        }
    }
}

fn entry_id(index: usize) -> iced::widget::Id {
    iced::widget::Id::from(format!("home-entry-{index}"))
}

/// Removes keyboard focus from whatever widget has it (i.e. the search input)
fn unfocus_search() -> Task<Message> {
    use iced::advanced::widget::{operate, operation::focusable};

    operate(focusable::unfocus::<()>()).discard()
}

/// View of the header of this screen
fn header_view<'a>(entry_count: usize, search: Option<&'a str>) -> impl Widget<Message> {
    // While searching, the input replaces the action buttons so nothing gets pushed off-screen
    let actions: Element<'a, Message> = match search {
        None => row![
            button(icons::get_icon("system-search-symbolic", 21).style(style::icon))
                .on_press(Message::OpenSearch)
                .padding(8)
                .style(style::transparent_button),
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
        .boxed(),
        Some(query) => search_input_view(query).boxed(),
    };

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
        container(actions).align_right(Length::Fill),
    ]
    .spacing(style::spacing::LARGE)
    .padding(10)
    .align_y(iced::Alignment::Center)
    .width(Length::Fill)
}

/// Entries matching the current search query (all of them if there's none)
fn filter_entries<'a>(
    entries: &'a [ClockodeEntry],
    search: Option<&str>,
) -> Vec<&'a ClockodeEntry> {
    let query = search
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .map(str::to_lowercase);

    entries
        .iter()
        .filter(|entry| {
            query
                .as_ref()
                .is_none_or(|query| entry.name.to_lowercase().contains(query))
        })
        .collect()
}

/// View of the contents of this screen
fn content_view<'a>(
    entries: &'a [ClockodeEntry],
    search: Option<&'a str>,
    selected: Option<usize>,
) -> Element<'a, Message> {
    let filtered = filter_entries(entries, search);

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
        .boxed()
    } else if filtered.is_empty() {
        container(
            text("No entries match your search")
                .size(style::font_size::BODY)
                .style(style::muted_text),
        )
        .center(Length::Fill)
        .boxed()
    } else {
        let entries_list = filtered.into_iter().enumerate().fold(
            Column::<Element<'_, Message>>::new()
                .height(Length::Fill)
                .spacing(style::spacing::MEDIUM)
                .padding(10),
            |col, (index, entry)| {
                let code = entry.totp.generate_current().to_string();
                let time_remaining = get_time_until_next_totp_refresh(entry.totp.step());
                let is_selected = selected == Some(index);

                let entry_view = container(
                    row![
                        column![
                            text(&entry.name)
                                .wrapping(text::Wrapping::Glyph)
                                .size(style::font_size::LARGE),
                            row![
                                text(format!(
                                    "{} digits · {}s",
                                    entry.totp.digits(),
                                    time_remaining
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
                .id(entry_id(index))
                .style(move |theme| {
                    if is_selected {
                        style::entry_card_selected(theme)
                    } else {
                        style::entry_card(theme)
                    }
                });

                col.push(entry_view.boxed())
            },
        );

        scrollable(entries_list)
            .height(Length::Fill)
            .id(ENTRIES_SCROLLABLE)
            .boxed()
    }
}

/// Search text input with an inline close button, shown in the header while searching
fn search_input_view(query: &str) -> impl Widget<Message> {
    const CLOSE_ICON_SIZE: u16 = 16;
    const CLOSE_PADDING: f32 = 4.0;

    let input = text_input("Search entries...", query)
        .id(SEARCH_INPUT)
        .on_input(Message::SearchChanged)
        // Extra right padding so typed text never runs under the close button
        .padding(
            iced::padding::all(8)
                .right(CLOSE_ICON_SIZE as f32 + CLOSE_PADDING * 2.0 + style::spacing::TINY * 2.0),
        )
        .width(Length::Fill.max(200.0))
        .style(style::text_input_style);

    let close =
        button(icons::get_icon("window-close-symbolic", CLOSE_ICON_SIZE).style(style::icon))
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
}

/// Scrolls the entry list just enough to make the entry at `index` fully visible
fn scroll_to_entry(index: usize) -> Task<Message> {
    use iced::advanced::widget::operation::Outcome;
    use iced::advanced::widget::{Id, Operation, operate};

    struct ScrollDelta {
        target: Id,
        delta: Option<f32>,
    }

    impl Operation<f32> for ScrollDelta {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<f32>)) {
            operate(self);
        }

        fn container(&mut self, id: Option<&Id>, bounds: Rectangle, viewport: &Rectangle) {
            if id != Some(&self.target) {
                return;
            }

            let margin = style::spacing::MEDIUM;
            let top = bounds.y - margin;
            let bottom = bounds.y + bounds.height + margin;

            self.delta = Some(if top < viewport.y {
                top - viewport.y
            } else if bottom > viewport.y + viewport.height {
                bottom - (viewport.y + viewport.height)
            } else {
                0.0
            });
        }

        fn finish(&self) -> Outcome<f32> {
            self.delta.map_or(Outcome::None, Outcome::Some)
        }
    }

    operate(ScrollDelta {
        target: entry_id(index),
        delta: None,
    })
    .then(|delta| {
        if delta == 0.0 {
            Task::none()
        } else {
            operation::scrollable::scroll_by(
                ENTRIES_SCROLLABLE,
                operation::scrollable::AbsoluteOffset { x: 0.0, y: delta },
                operation::Animation::Auto,
            )
        }
    })
}
