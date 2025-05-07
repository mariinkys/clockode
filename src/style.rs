use iced::{
    Theme,
    widget::{button, container},
};

pub const DEFAULT_BORDER_RADIUS: f32 = 12.0;

pub fn rounded_primary_button(t: &Theme, s: button::Status) -> button::Style {
    let mut style = iced::widget::button::primary(t, s);
    style.border.radius = iced::border::radius(DEFAULT_BORDER_RADIUS);
    style
}

pub fn rounded_danger_button(t: &Theme, s: button::Status) -> button::Style {
    let mut style = iced::widget::button::danger(t, s);
    style.border.radius = iced::border::radius(DEFAULT_BORDER_RADIUS);
    style
}

pub fn rounded_container(t: &Theme) -> container::Style {
    let mut style = container::rounded_box(t);
    style.border.radius = iced::border::radius(DEFAULT_BORDER_RADIUS);
    style
}
