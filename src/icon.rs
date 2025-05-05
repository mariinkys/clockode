// Generated automatically by iced_fontello at build time.
// Do not edit manually. Source: ../fonts/iced-twofa.toml
// 5024f28ee87956dff0128fa42c0e534ff52be1435667b0b37bfc82cbda59f611
use iced::widget::{text, Text};
use iced::Font;

pub const FONT: &[u8] = include_bytes!("../fonts/iced-twofa.ttf");

pub fn add<'a>() -> Text<'a> {
    icon("\u{2B}")
}

pub fn cancel<'a>() -> Text<'a> {
    icon("\u{2715}")
}

pub fn config<'a>() -> Text<'a> {
    icon("\u{2699}")
}

pub fn edit<'a>() -> Text<'a> {
    icon("\u{270E}")
}

pub fn expand<'a>() -> Text<'a> {
    icon("\u{1F4D5}")
}

pub fn export<'a>() -> Text<'a> {
    icon("\u{E715}")
}

pub fn save<'a>() -> Text<'a> {
    icon("\u{1F4BE}")
}

pub fn trash<'a>() -> Text<'a> {
    icon("\u{F1F8}")
}

pub fn unlock<'a>() -> Text<'a> {
    icon("\u{1F513}")
}

pub fn user<'a>() -> Text<'a> {
    icon("\u{1F464}")
}

fn icon(codepoint: &str) -> Text<'_> {
    text(codepoint).font(Font::with_name("iced-twofa"))
}
