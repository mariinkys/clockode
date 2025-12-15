// SPDX-License-Identifier: GPL-3.0-only
#![windows_subsystem = "windows"]

use crate::app::Clockode;

mod app;
mod icons;

const APP_ID: &str = "dev.mariinkys.Clockode";
const APP_ICON: &[u8] = include_bytes!("../resources/icons/hicolor/scalable/apps/icon.svg");

fn main() -> iced::Result {
    // Init the icon cache
    icons::ICON_CACHE.get_or_init(|| std::sync::Mutex::new(icons::IconCache::new()));

    let app_icon = iced::window::icon::from_file_data(
        include_bytes!("../resources/icons/hicolor/scalable/apps/icon.svg"),
        None,
    );

    let platform_settings = {
        #[cfg(target_os = "linux")]
        {
            iced::window::settings::PlatformSpecific {
                application_id: String::from(APP_ID),
                ..Default::default()
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            Default::default()
        }
    };

    iced::application::timed(
        Clockode::new,
        Clockode::update,
        Clockode::subscription,
        Clockode::view,
    )
    .theme(Clockode::theme)
    .window(iced::window::Settings {
        min_size: Some(iced::Size {
            width: 300.,
            height: 400.,
        }),
        icon: app_icon.ok(),
        platform_specific: platform_settings,
        ..Default::default()
    })
    .run()
}
