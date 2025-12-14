use iced::{Element, Subscription, Task, Theme, time::Instant, widget::text};

pub struct Clockode {
    now: Instant,
}

#[derive(Debug, Clone)]
pub enum Message {}

impl Clockode {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                now: Instant::now(),
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, message: Message, now: Instant) -> Task<Message> {
        self.now = now;
        match message {}
    }

    pub fn view(&self) -> Element<'_, Message> {
        text("Hello!").into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    pub fn theme(&self) -> Theme {
        Theme::Light
    }
}
