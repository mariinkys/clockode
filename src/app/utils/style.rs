use iced::widget::{button, container, svg, text, text_input};
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
        background: Some(theme.palette().background.base.color.into()),
        border: Border {
            color: theme.palette().background.weak.text.scale_alpha(0.1),
            width: 1.0,
            radius: radius::LARGE.into(),
        },
        ..Default::default()
    }
}

/// Entry card style - for TOTP entry items
pub fn entry_card(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(theme.palette().background.base.color.into()),
        border: Border {
            color: theme.palette().background.weak.text.scale_alpha(0.1),
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

/// Transparent button style: no background, no border, no shadow. Only the content is visible
pub fn transparent_button(theme: &Theme, status: button::Status) -> button::Style {
    let text_color = theme.palette().background.base.text;

    button::Style {
        background: None,
        text_color: match status {
            button::Status::Active => text_color,
            button::Status::Hovered => text_color.scale_alpha(0.7),
            button::Status::Pressed => text_color.scale_alpha(0.5),
            button::Status::Disabled => text_color.scale_alpha(0.4),
        },
        ..button::Style::default()
    }
}

/// Label text style (subdued color)
pub fn label_text(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().background.weak.text.scale_alpha(0.8)),
    }
}

/// Muted text style (for hints, subtitles, etc.)
pub fn muted_text(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().background.weak.text.scale_alpha(0.6)),
    }
}

/// Link text style (for clickable urls...)
pub fn link_text(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().background.weak.text.scale_alpha(0.8)),
    }
}

/// Subtitle text style (slightly muted)
pub fn subtitle_text(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().background.weak.text.scale_alpha(0.7)),
    }
}

/// Icon on a transparent surface: same color as regular text
pub fn icon(theme: &Theme, status: svg::Status) -> svg::Style {
    tinted(theme.palette().background.base.text, status)
}

#[allow(dead_code)]
/// Icon on a primary button
pub fn icon_on_primary(theme: &Theme, _status: svg::Status) -> svg::Style {
    svg::Style {
        color: Some(theme.palette().primary.base.text),
    }
}

#[allow(dead_code)]
/// Icon on a secondary button
pub fn icon_on_secondary(theme: &Theme, _status: svg::Status) -> svg::Style {
    svg::Style {
        color: Some(theme.palette().secondary.base.text),
    }
}

fn tinted(color: iced::Color, status: svg::Status) -> svg::Style {
    svg::Style {
        color: Some(match status {
            svg::Status::Idle => color,
            svg::Status::Hovered => color.scale_alpha(0.7),
        }),
    }
}

/// Text input style matching the cards and buttons
pub fn text_input_style(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let palette = theme.palette();
    let base = text_input::default(theme, status);

    let border_color = match status {
        text_input::Status::Focused { .. } => palette.primary.base.color,
        text_input::Status::Hovered => palette.background.base.text.scale_alpha(0.25),
        text_input::Status::Active | text_input::Status::Disabled => {
            palette.background.base.text.scale_alpha(0.1)
        }
    };

    text_input::Style {
        background: palette.background.base.color.into(),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: radius::SMALL.into(),
        },
        placeholder: palette.background.base.text.scale_alpha(0.6),
        ..base
    }
}
