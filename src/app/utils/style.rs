use iced::widget::{button, container, text};
use iced::{Border, Theme};

/// Standard spacing values
pub mod spacing {
    pub const TINY: f32 = 4.0;
    pub const SMALL: f32 = 8.0;
    pub const MEDIUM: f32 = 12.0;
    pub const LARGE: f32 = 16.0;
    pub const XLARGE: f32 = 20.0;
}

/// Standard border radius values
pub mod radius {
    pub const SMALL: f32 = 6.0;
    pub const MEDIUM: f32 = 8.0;
    pub const LARGE: f32 = 12.0;
}

/// Standard font sizes
pub mod font_size {
    pub const SMALL: f32 = 12.0;
    pub const BODY: f32 = 14.0;
    pub const MEDIUM: f32 = 16.0;
    pub const LARGE: f32 = 18.0;
    //pub const XLARGE: f32 = 20.0;
    pub const TITLE: f32 = 24.0;
    pub const HERO: f32 = 28.0;
}

/// Card container style - used for entry cards, form containers, etc.
pub fn card_container(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(theme.palette().background.into()),
        border: Border {
            color: theme.palette().text.scale_alpha(0.1),
            width: 1.0,
            radius: radius::LARGE.into(),
        },
        ..Default::default()
    }
}

/// Entry card style - for TOTP entry items
pub fn entry_card(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(theme.palette().background.into()),
        border: Border {
            color: theme.palette().text.scale_alpha(0.1),
            width: 1.0,
            radius: radius::MEDIUM.into(),
        },
        ..Default::default()
    }
}

/// Primary submit button style
pub fn primary_submit_button(theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        border: Border {
            radius: radius::MEDIUM.into(),
            ..Default::default()
        },
        ..button::primary(theme, status)
    }
}

/// Primary button style
pub fn primary_button(theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        border: Border {
            radius: radius::SMALL.into(),
            ..Default::default()
        },
        ..button::primary(theme, status)
    }
}

/// Secondary button style with rounded corners
pub fn secondary_button(theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        border: Border {
            radius: radius::SMALL.into(),
            ..Default::default()
        },
        ..button::secondary(theme, status)
    }
}

/// Danger button style with rounded corners
pub fn danger_button(theme: &Theme, status: button::Status) -> button::Style {
    let mut style = button::danger(theme, status);
    style.border = iced::Border {
        radius: radius::SMALL.into(),
        ..Default::default()
    };
    style
}

/// Success button style with rounded corners
pub fn success_button(theme: &Theme, status: button::Status) -> button::Style {
    let mut style = button::success(theme, status);
    style.border = iced::Border {
        radius: radius::SMALL.into(),
        ..Default::default()
    };
    style
}

/// Label text style (subdued color)
pub fn label_text(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().text.scale_alpha(0.8)),
    }
}

/// Muted text style (for hints, subtitles, etc.)
pub fn muted_text(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().text.scale_alpha(0.6)),
    }
}

/// Link text style (for clickable urls...)
pub fn link_text(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().primary.scale_alpha(0.8)),
    }
}

/// Subtitle text style (slightly muted)
pub fn subtitle_text(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().text.scale_alpha(0.7)),
    }
}
