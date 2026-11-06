//! Persistent, versioned settings; unknown fields load as defaults.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::Favorite;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemePref {
    #[default]
    System,
    Light,
    Dark,
}

/// Where the window controls are drawn; `Auto` follows the platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WindowControlsPref {
    #[default]
    Auto,
    Left,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub theme: ThemePref,
    pub window_controls: WindowControlsPref,
    pub translucent_window: bool,
    pub adb_path: Option<String>,
    pub scrcpy_path: Option<String>,
    pub concurrent_transfers: usize,
    pub show_hidden: bool,
    pub open_on_single_click: bool,
    pub confirm_before_delete: bool,
    pub default_download_folder: String,
    pub favorites: Vec<Favorite>,
}

impl Default for Settings {
    fn default() -> Self {
        let default_download_folder = dirs::download_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join("Downloads")))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        Self {
            version: 2,
            theme: ThemePref::System,
            window_controls: WindowControlsPref::Auto,
            translucent_window: true,
            adb_path: None,
            scrcpy_path: None,
            concurrent_transfers: 4,
            show_hidden: false,
            open_on_single_click: false,
            confirm_before_delete: true,
            default_download_folder,
            favorites: Vec::new(),
        }
    }
}

impl Settings {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("AndroidFiles")
            .join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let mut settings: Self = serde_json::from_str(&text).unwrap_or_default();
                // Pre-v2 files carry the old fixed theme default.
                if settings.version < 2 {
                    settings.theme = ThemePref::System;
                    settings.version = 2;
                }
                settings
            }
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }
}
