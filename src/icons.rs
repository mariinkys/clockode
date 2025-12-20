// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use iced::widget::svg;

pub(crate) static ICON_CACHE: OnceLock<Mutex<IconCache>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IconCacheKey {
    name: &'static str,
    size: u16,
}

pub struct IconCache {
    cache: HashMap<IconCacheKey, svg::Handle>,
}

impl IconCache {
    pub fn new() -> Self {
        let mut cache = HashMap::new();

        macro_rules! bundle {
            ($name:expr, $size:expr) => {
                let data: &'static [u8] =
                    include_bytes!(concat!("../resources/icons/bundled/", $name, ".svg"));
                cache.insert(
                    IconCacheKey {
                        name: $name,
                        size: $size,
                    },
                    svg::Handle::from_memory(data),
                );
            };
        }

        bundle!("edit-symbolic", 21);
        bundle!("emblem-system-symbolic", 21);
        bundle!("list-add-symbolic", 21);
        bundle!("edit-copy-symbolic", 21);
        bundle!("go-previous-symbolic", 21);
        bundle!("list-add-symbolic", 21);
        bundle!("user-trash-full-symbolic", 21);
        bundle!("document-export-symbolic", 21);
        bundle!("document-import-symbolic", 21);
        bundle!("window-close-symbolic", 21);
        bundle!("qr-symbolic", 21);
        bundle!("camera-photo-symbolic", 48);

        Self { cache }
    }

    fn get_handle(&mut self, name: &'static str, size: u16) -> svg::Handle {
        self.cache
            .entry(IconCacheKey { name, size })
            .or_insert_with(|| svg::Handle::from_memory(name.as_bytes()))
            .clone()
    }
}

pub fn get_icon(name: &'static str, size: u16) -> svg::Svg<'static> {
    let handle = {
        let mut icon_cache = ICON_CACHE.get().unwrap().lock().unwrap();
        icon_cache.get_handle(name, size)
    };

    svg::Svg::new(handle)
        .width(iced::Length::Fixed(size.into()))
        .height(iced::Length::Fixed(size.into()))
}
