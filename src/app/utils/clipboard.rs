// SPDX-License-Identifier: GPL-3.0-only

use iced::{Task, window};
use tracing::{error, info};

pub enum AppClipboard {
    Pending,
    #[cfg(target_os = "linux")]
    Wayland(smithay_clipboard::Clipboard),
    Arboard(arboard::Clipboard),
    Unavailable,
}

impl AppClipboard {
    /// `wayland_display` is the `wl_display*` of the main window, if it runs on Wayland.
    pub fn init(&mut self, wayland_display: Option<usize>) {
        *self = Self::wayland(wayland_display).unwrap_or_else(|| {
            match arboard::Clipboard::new() {
                Ok(clipboard) => {
                    info!("Using arboard clipboard");
                    Self::Arboard(clipboard)
                }
                Err(err) => {
                    error!("{err}");
                    Self::Unavailable
                }
            }
        });
    }

    #[cfg(target_os = "linux")]
    fn wayland(display: Option<usize>) -> Option<Self> {
        let display = display?;
        // SAFETY: the display belongs to winit's connection, which outlives
        // the app state that owns this clipboard.
        let clipboard =
            unsafe { smithay_clipboard::Clipboard::new(display as *mut std::ffi::c_void) };
        info!("Using Wayland clipboard");
        Some(Self::Wayland(clipboard))
    }

    #[cfg(not(target_os = "linux"))]
    fn wayland(_: Option<usize>) -> Option<Self> {
        None
    }

    pub fn set_text(&mut self, text: String) -> Result<(), String> {
        match self {
            Self::Pending => Err("Clipboard is not ready yet".into()),
            #[cfg(target_os = "linux")]
            Self::Wayland(clipboard) => {
                clipboard.store(text); // fire-and-forget, reports no errors
                Ok(())
            }
            Self::Arboard(clipboard) => clipboard.set_text(text).map_err(|e| e.to_string()),
            Self::Unavailable => Err("Clipboard is unavailable".into()),
        }
    }
}

/// Asks iced for the main window's `wl_display` pointer (`None` if not on Wayland).
pub fn resolve_display() -> Task<Option<usize>> {
    window::oldest().then(|id| match id {
        Some(id) => window::run(id, |window| {
            use raw_window_handle::RawDisplayHandle;

            match window.display_handle().ok()?.as_raw() {
                RawDisplayHandle::Wayland(handle) => Some(handle.display.as_ptr() as usize),
                _ => None,
            }
        }),
        None => Task::done(None),
    })
}
