//! Pastel: a Win+V style clipboard history manager for Linux.
//!
//! Entry point: handles the `show`, `quit`, `install` and `update` commands,
//! makes sure only one instance runs, then starts the GTK app.

mod backend;
mod format;
mod models;
mod paths;
mod ui;

use crate::paths::{applications_path, desktop_entry, exe_path, socket_path};
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::time::Duration;

const INSTALL_URL: &str = "https://raw.githubusercontent.com/raheem-dotgit/pastel/main/install.sh";
const MEDIA_KEYS: &str = "org.gnome.settings-daemon.plugins.media-keys";
const SHORTCUT_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/pastel/";

/// Send a command to the running instance. Returns false if none is running.
fn send(msg: &[u8]) -> bool {
    UnixStream::connect(socket_path())
        .and_then(|mut s| s.write_all(msg))
        .is_ok()
}

/// Show the running instance's window, starting Pastel first if needed.
fn cmd_show() {
    if send(b"show") {
        return;
    }
    if let Ok(exe) = std::env::current_exe() {
        let _ = Command::new(exe)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
    std::thread::sleep(Duration::from_millis(900));
    send(b"show");
}

/// Run `gsettings` and return its trimmed output on success.
fn gsettings(args: &[&str]) -> Option<String> {
    let out = Command::new("gsettings").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Parse a GVariant string array such as `['a', 'b']` or `@as []`.
fn parse_list(s: &str) -> Vec<String> {
    s.split('\'').skip(1).step_by(2).map(String::from).collect()
}

fn format_list(items: &[String]) -> String {
    if items.is_empty() {
        return "@as []".into();
    }
    let quoted: Vec<String> = items.iter().map(|i| format!("'{i}'")).collect();
    format!("[{}]", quoted.join(", "))
}

/// Bind Super+V to `pastel show` in GNOME. GNOME uses Super+V for the
/// notification list by default, so that binding is removed (Super+M still
/// opens it). Returns None when GNOME settings are unavailable.
fn bind_super_v() -> Option<()> {
    let tray = parse_list(&gsettings(&[
        "get",
        "org.gnome.shell.keybindings",
        "toggle-message-tray",
    ])?);
    let tray: Vec<String> = tray
        .into_iter()
        .filter(|k| !k.eq_ignore_ascii_case("<Super>v"))
        .collect();
    gsettings(&[
        "set",
        "org.gnome.shell.keybindings",
        "toggle-message-tray",
        &format_list(&tray),
    ])?;

    let mut paths = parse_list(&gsettings(&["get", MEDIA_KEYS, "custom-keybindings"])?);
    if !paths.iter().any(|p| p == SHORTCUT_PATH) {
        paths.push(SHORTCUT_PATH.into());
    }
    gsettings(&[
        "set",
        MEDIA_KEYS,
        "custom-keybindings",
        &format_list(&paths),
    ])?;

    let schema = format!("{MEDIA_KEYS}.custom-keybinding:{SHORTCUT_PATH}");
    let command = format!("'{} show'", exe_path());
    gsettings(&["set", &schema, "name", "'Pastel'"])?;
    gsettings(&["set", &schema, "command", &command])?;
    gsettings(&["set", &schema, "binding", "'<Super>v'"])?;
    Some(())
}

/// Add Pastel to the app menu and bind Super+V.
fn cmd_install() {
    let path = applications_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(&path, desktop_entry(&exe_path())) {
        Ok(_) => println!("Added to the app menu: {}", path.display()),
        Err(e) => eprintln!("Failed to add the app menu entry: {e}"),
    }
    match bind_super_v() {
        Some(()) => println!("Bound Super+V to open Pastel."),
        None => println!(
            "Could not set the shortcut automatically (GNOME only). \
             Bind '{} show' to Super+V in your desktop's keyboard settings.",
            exe_path()
        ),
    }
}

/// Update to the latest release by re-running the install script.
fn cmd_update() {
    // Download the script fully first, so a failed download runs nothing.
    let script = format!("set -e; s=$(curl -fsSL {INSTALL_URL}); printf '%s\\n' \"$s\" | sh");
    let ok = Command::new("sh")
        .args(["-c", &script])
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        eprintln!("Update failed. Check your internet connection and that curl is installed.");
        std::process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("show") => return cmd_show(),
        Some("quit") => {
            send(b"quit");
            return;
        }
        Some("install") => return cmd_install(),
        Some("update") => return cmd_update(),
        _ => {}
    }

    if send(b"show") {
        println!("Pastel is already running; showing its window.");
        return;
    }
    let _ = std::fs::remove_file(socket_path());

    ui::main();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gvariant_lists_round_trip() {
        assert!(parse_list("@as []").is_empty());
        let items = parse_list("['<Super>v', '<Super>m']");
        assert_eq!(items, ["<Super>v", "<Super>m"]);
        assert_eq!(format_list(&items), "['<Super>v', '<Super>m']");
        assert_eq!(format_list(&[]), "@as []");
    }
}
