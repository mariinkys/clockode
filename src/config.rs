// SPDX-License-Identifier: GPL-3.0-only

use anywho::anywho;
use iced::Theme;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub theme: ColockodeTheme,
}

impl Config {
    pub async fn load(app_id: &str) -> Result<Self, anywho::Error> {
        use dirs;
        use std::fs;

        let app_id = app_id.to_string();

        smol::unblock(move || {
            let config_dir = dirs::data_dir()
                .ok_or_else(|| anywho!("Could not determine config directory"))?
                .join(&app_id);

            // create config directory if it doesn't exist
            if !config_dir.exists() {
                fs::create_dir_all(&config_dir)
                    .map_err(|e| anywho!("Failed to create config directory: {}", e))?;
            }

            let config_path = config_dir.join("config.ron");

            if config_path.exists() {
                let config_content = fs::read_to_string(&config_path)
                    .map_err(|e| anywho!("Failed to read config file: {}", e))?;

                ron::from_str(&config_content)
                    .map_err(|e| anywho!("Failed to parse config file: {}", e))
            } else {
                let config = Config::default();

                // Save default config
                let config_content =
                    ron::ser::to_string_pretty(&config, ron::ser::PrettyConfig::default())
                        .map_err(|e| anywho!("Failed to serialize config: {}", e))?;

                fs::write(&config_path, config_content)
                    .map_err(|e| anywho!("Failed to write config file: {}", e))?;

                Ok(config)
            }
        })
        .await
        .map_err(|e| anywho!("Error loading config: {}", e))
    }

    pub async fn save(self, app_id: &str) -> Result<(), anywho::Error> {
        use dirs;
        use std::fs;

        let config_clone = self.clone();
        let app_id = app_id.to_string();

        smol::unblock(move || {
            let config_dir = dirs::data_dir()
                .ok_or_else(|| anywho!("Could not determine config directory"))?
                .join(&app_id);

            if !config_dir.exists() {
                fs::create_dir_all(&config_dir)
                    .map_err(|e| anywho!("Failed to create config directory: {}", e))?;
            }

            let config_path = config_dir.join("config.ron");

            let config_content =
                ron::ser::to_string_pretty(&config_clone, ron::ser::PrettyConfig::default())
                    .map_err(|e| anywho!("Failed to serialize config: {}", e))?;

            fs::write(&config_path, config_content)
                .map_err(|e| anywho!("Failed to write config file: {}", e))?;

            Ok(())
        })
        .await
        .map_err(|e: anywho::Error| anywho!("Unblock error: {}", e))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum ColockodeTheme {
    Light,
    Dark,
    Dracula,
    Nord,
    SolarizedLight,
    SolarizedDark,
    GruvboxLight,
    GruvboxDark,
    CatppuccinLatte,
    CatppuccinFrappe,
    #[default]
    CatppuccinMacchiato,
    CatppuccinMocha,
    TokyoNight,
    TokyoNightStorm,
    TokyoNightLight,
    KanagawaWave,
    KanagawaDragon,
    KanagawaLotus,
    Moonfly,
    Nightfly,
    Oxocarbon,
    Ferra,
}

impl From<ColockodeTheme> for Theme {
    fn from(config_theme: ColockodeTheme) -> Self {
        match config_theme {
            ColockodeTheme::Light => Theme::Light,
            ColockodeTheme::Dark => Theme::Dark,
            ColockodeTheme::Dracula => Theme::Dracula,
            ColockodeTheme::Nord => Theme::Nord,
            ColockodeTheme::SolarizedLight => Theme::SolarizedLight,
            ColockodeTheme::SolarizedDark => Theme::SolarizedDark,
            ColockodeTheme::GruvboxLight => Theme::GruvboxLight,
            ColockodeTheme::GruvboxDark => Theme::GruvboxDark,
            ColockodeTheme::CatppuccinLatte => Theme::CatppuccinLatte,
            ColockodeTheme::CatppuccinFrappe => Theme::CatppuccinFrappe,
            ColockodeTheme::CatppuccinMacchiato => Theme::CatppuccinMacchiato,
            ColockodeTheme::CatppuccinMocha => Theme::CatppuccinMocha,
            ColockodeTheme::TokyoNight => Theme::TokyoNight,
            ColockodeTheme::TokyoNightStorm => Theme::TokyoNightStorm,
            ColockodeTheme::TokyoNightLight => Theme::TokyoNightLight,
            ColockodeTheme::KanagawaWave => Theme::KanagawaWave,
            ColockodeTheme::KanagawaDragon => Theme::KanagawaDragon,
            ColockodeTheme::KanagawaLotus => Theme::KanagawaLotus,
            ColockodeTheme::Moonfly => Theme::Moonfly,
            ColockodeTheme::Nightfly => Theme::Nightfly,
            ColockodeTheme::Oxocarbon => Theme::Oxocarbon,
            ColockodeTheme::Ferra => Theme::Ferra,
        }
    }
}

/// Will fail for custom themes
impl TryFrom<&Theme> for ColockodeTheme {
    type Error = &'static str;

    fn try_from(theme: &Theme) -> Result<Self, Self::Error> {
        match theme {
            Theme::Light => Ok(ColockodeTheme::Light),
            Theme::Dark => Ok(ColockodeTheme::Dark),
            Theme::Dracula => Ok(ColockodeTheme::Dracula),
            Theme::Nord => Ok(ColockodeTheme::Nord),
            Theme::SolarizedLight => Ok(ColockodeTheme::SolarizedLight),
            Theme::SolarizedDark => Ok(ColockodeTheme::SolarizedDark),
            Theme::GruvboxLight => Ok(ColockodeTheme::GruvboxLight),
            Theme::GruvboxDark => Ok(ColockodeTheme::GruvboxDark),
            Theme::CatppuccinLatte => Ok(ColockodeTheme::CatppuccinLatte),
            Theme::CatppuccinFrappe => Ok(ColockodeTheme::CatppuccinFrappe),
            Theme::CatppuccinMacchiato => Ok(ColockodeTheme::CatppuccinMacchiato),
            Theme::CatppuccinMocha => Ok(ColockodeTheme::CatppuccinMocha),
            Theme::TokyoNight => Ok(ColockodeTheme::TokyoNight),
            Theme::TokyoNightStorm => Ok(ColockodeTheme::TokyoNightStorm),
            Theme::TokyoNightLight => Ok(ColockodeTheme::TokyoNightLight),
            Theme::KanagawaWave => Ok(ColockodeTheme::KanagawaWave),
            Theme::KanagawaDragon => Ok(ColockodeTheme::KanagawaDragon),
            Theme::KanagawaLotus => Ok(ColockodeTheme::KanagawaLotus),
            Theme::Moonfly => Ok(ColockodeTheme::Moonfly),
            Theme::Nightfly => Ok(ColockodeTheme::Nightfly),
            Theme::Oxocarbon => Ok(ColockodeTheme::Oxocarbon),
            Theme::Ferra => Ok(ColockodeTheme::Ferra),
            Theme::Custom(_) => Err("Custom themes cannot be converted to ConfigTheme"),
        }
    }
}
