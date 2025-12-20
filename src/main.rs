// SPDX-License-Identifier: GPL-3.0-only
#![windows_subsystem = "windows"]

use crate::app::Clockode;

mod app;
mod config;
mod icons;

const APP_ID: &str = "dev.mariinkys.Clockode";
const APP_ICON: &[u8] = include_bytes!("../resources/icons/hicolor/scalable/apps/icon.svg");

/// SEE: https://github.com/pop-os/cosmic-bg/pull/73
/// Access glibc malloc tunables.
#[cfg(target_env = "gnu")]
mod malloc {
    use std::os::raw::c_int;
    const M_MMAP_THRESHOLD: c_int = -3;

    unsafe extern "C" {
        fn mallopt(param: c_int, value: c_int) -> c_int;
    }

    /// Prevents glibc from hoarding memory via memory fragmentation.
    pub fn limit_mmap_threshold() {
        unsafe {
            mallopt(M_MMAP_THRESHOLD, 65536);
        }
    }
}

fn main() -> iced::Result {
    // Prevents glibc from hoarding memory via memory fragmentation.
    #[cfg(target_env = "gnu")]
    malloc::limit_mmap_threshold();

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
    .title("Clockode")
    .theme(Clockode::theme)
    .window_size(iced::Size {
        width: 1100.,
        height: 700.,
    })
    .window(iced::window::Settings {
        min_size: Some(iced::Size {
            width: 450.,
            height: 500.,
        }),
        icon: app_icon.ok(),
        platform_specific: platform_settings,
        ..Default::default()
    })
    .run()
}
