//! Data types and JSON persistence.

use crate::paths::{history_path, settings_path};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct ClipItem {
    pub text: String,
    pub pinned: bool,
    pub ts: f64,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum CloseAction {
    Hide,
    Quit,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppTheme {
    #[default]
    SoftDark,
    SoftLight,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Settings {
    pub max_items: usize,
    pub hide_after_copy: bool,
    pub close_action: CloseAction,
    pub autostart: bool,
    #[serde(default)]
    pub theme: AppTheme,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_items: 100,
            hide_after_copy: true,
            close_action: CloseAction::Hide,
            autostart: false,
            theme: AppTheme::SoftDark,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(settings_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
}

#[derive(Serialize, Deserialize)]
pub struct HistoryFile {
    pub items: Vec<ClipItem>,
}

pub fn load_history() -> Vec<ClipItem> {
    std::fs::read_to_string(history_path())
        .ok()
        .and_then(|s| serde_json::from_str::<HistoryFile>(&s).ok())
        .map(|f| f.items)
        .unwrap_or_default()
}

pub fn save_history(items: &[ClipItem]) {
    let hist = HistoryFile {
        items: items.to_vec(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&hist) {
        let _ = std::fs::write(history_path(), json);
    }
}

pub fn save_settings(settings: &Settings) {
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(settings_path(), json);
    }
}
