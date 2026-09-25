//! Data types and JSON persistence.

use crate::paths::{history_path, images_dir, settings_path};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone)]
pub struct ClipItem {
    /// The clip text, or a label such as "Image · 800×600" for images.
    pub text: String,
    pub pinned: bool,
    pub ts: f64,
    /// For images: the file stem under `images/` (`<id>.png`, `<id>-thumb.png`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

impl ClipItem {
    /// Identity used to spot repeat copies: the image id, or the text.
    pub fn key(&self) -> &str {
        self.image.as_deref().unwrap_or(&self.text)
    }
}

pub fn image_path(id: &str) -> PathBuf {
    images_dir().join(format!("{id}.png"))
}

pub fn thumb_path(id: &str) -> PathBuf {
    images_dir().join(format!("{id}-thumb.png"))
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

#[derive(Deserialize)]
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

/// Write a file readable only by the user, via a temp file and rename so a
/// crash mid-write never leaves a truncated history behind.
pub fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    file.write_all(contents)?;
    file.sync_all()?;
    std::fs::rename(&tmp, path)
}

pub fn save_history(items: &[ClipItem]) {
    #[derive(Serialize)]
    struct HistoryRef<'a> {
        items: &'a [ClipItem],
    }
    if let Ok(json) = serde_json::to_string_pretty(&HistoryRef { items }) {
        let _ = write_private(&history_path(), json.as_bytes());
    }
}

pub fn save_settings(settings: &Settings) {
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = write_private(&settings_path(), json.as_bytes());
    }
}

/// Delete image files no longer referenced by any history item.
pub fn prune_images(items: &[ClipItem]) {
    let Ok(entries) = std::fs::read_dir(images_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let id = name.trim_end_matches(".png").trim_end_matches("-thumb");
        if !items.iter().any(|i| i.image.as_deref() == Some(id)) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn write_private_is_user_only_and_complete() {
        let path = std::env::temp_dir().join(format!("pastel-test-{}.json", std::process::id()));
        write_private(&path, b"{\"a\":1}").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"a\":1}");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(!path.with_extension("tmp").exists());
        std::fs::remove_file(path).unwrap();
    }
}
