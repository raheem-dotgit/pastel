//! XDG paths and desktop-entry helpers.

use std::path::PathBuf;

fn config_home() -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join(".config")
        })
}

pub fn config_dir() -> PathBuf {
    config_home().join("pastel")
}

pub fn history_path() -> PathBuf {
    config_dir().join("history.json")
}

pub fn images_dir() -> PathBuf {
    config_dir().join("images")
}

pub fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn autostart_path() -> PathBuf {
    config_home().join("autostart").join("pastel.desktop")
}

pub fn applications_path() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
                .join(".local")
                .join("share")
        });
    base.join("applications").join("pastel.desktop")
}

pub fn socket_path() -> PathBuf {
    let base = std::env::var("XDG_RUNTIME_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    base.join(format!(
        "pastel-{}.sock",
        std::env::var("USER").unwrap_or_else(|_| "user".into())
    ))
}

pub fn exe_path() -> String {
    std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "pastel".into())
}

pub fn desktop_entry(exec: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Pastel\n\
         GenericName=Clipboard Manager\n\
         Comment=Clipboard history with search and pins\n\
         Keywords=clipboard;history;paste;copy;\n\
         Exec={exec}\n\
         Icon=edit-paste\n\
         Terminal=false\n\
         Categories=Utility;\n\
         StartupWMClass=io.github.raheem_dotgit.Pastel\n"
    )
}
