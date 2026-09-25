//! Clipboard backend: `wl-paste --watch` capture and `wl-copy` for text and images.

use gtk4::gio;
use gtk4::prelude::*;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;

/// One clipboard event from the background watcher.
pub enum WatchEvent {
    Clip(String),
    /// Raw image bytes in whatever format the source offered (PNG, JPEG, ...).
    /// The optional name is the source file name, when the copy was a file.
    Image(Vec<u8>, Option<String>),
    Dead,
}

/// True when the clipboard offers plain text.
pub fn has_text<S: AsRef<str>>(types: &[S]) -> bool {
    types.iter().any(|t| {
        let t = t.as_ref();
        t.starts_with("text/plain") || t == "UTF8_STRING" || t == "STRING" || t == "TEXT"
    })
}

/// The image type to read, preferring PNG. Only used when there is no text,
/// so copying a file or rich text is still recorded as text.
pub fn image_type<S: AsRef<str>>(types: &[S]) -> Option<&str> {
    let types: Vec<&str> = types.iter().map(|t| t.as_ref()).collect();
    if has_text(&types) {
        return None;
    }
    let pick = types
        .iter()
        .position(|t| *t == "image/png")
        .or_else(|| types.iter().position(|t| t.starts_with("image/")))?;
    Some(types[pick])
}

/// When the copied text is a single image file (a path, or a `file://` or
/// `recent://` link as the Files app copies it), return the file's path.
pub fn image_file(text: &str) -> Option<PathBuf> {
    let s = text.trim();
    if s.is_empty() || s.contains('\n') {
        return None;
    }
    let file = if s.starts_with('/') {
        gio::File::for_path(s)
    } else if s.starts_with("file://") || s.starts_with("recent://") {
        gio::File::for_uri(s)
    } else {
        return None;
    };
    let info = file
        .query_info(
            "standard::content-type,standard::target-uri",
            gio::FileQueryInfoFlags::NONE,
            None::<&gio::Cancellable>,
        )
        .ok()?;
    if !info.content_type()?.starts_with("image/") {
        return None;
    }
    match info.attribute_string("standard::target-uri") {
        Some(target) => gio::File::for_uri(&target).path(),
        None => file.path(),
    }
}

/// True when a password manager (KeePassXC etc.) marked the clip as secret.
pub fn is_secret<S: AsRef<str>>(types: &[S]) -> bool {
    types
        .iter()
        .any(|t| t.as_ref().contains("passwordManagerHint"))
}

pub fn file_name(path: &Path) -> Option<String> {
    Some(path.file_name()?.to_string_lossy().into_owned())
}

fn wl_paste(args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new("wl-paste").args(args).output().ok()?;
    out.status.success().then_some(out.stdout)
}

/// Read the current clipboard as a watcher event, or None to skip it.
fn read_current() -> Option<WatchEvent> {
    let list = String::from_utf8(wl_paste(&["--list-types"])?).ok()?;
    let types: Vec<&str> = list.lines().collect();
    if is_secret(&types) {
        return None;
    }
    if has_text(&types) {
        let text = wl_paste(&["--no-newline", "--type", "text"])?;
        let text = String::from_utf8_lossy(&text).into_owned();
        if let Some(path) = image_file(&text) {
            if let Ok(bytes) = std::fs::read(&path) {
                return Some(WatchEvent::Image(bytes, file_name(&path)));
            }
        }
        return Some(WatchEvent::Clip(text));
    }
    let bytes = wl_paste(&["--type", image_type(&types)?])?;
    Some(WatchEvent::Image(bytes, None))
}

/// Watch clipboard changes with `wl-paste --watch`, which prints one byte per
/// change; each change is then read with the right type (text or image).
pub fn spawn_wayland_watcher() -> Receiver<WatchEvent> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let child = Command::new("wl-paste")
            .args(["--watch", "sh", "-c", "cat >/dev/null; printf x"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(_) => {
                let _ = tx.send(WatchEvent::Dead);
                return;
            }
        };
        let Some(mut stdout) = child.stdout.take() else {
            let _ = tx.send(WatchEvent::Dead);
            return;
        };

        let mut buf = [0u8; 64];
        loop {
            match stdout.read(&mut buf) {
                Ok(0) | Err(_) => break,
                // Several changes can arrive in one read; the latest clip is enough.
                Ok(_) => {
                    let Some(event) = read_current() else {
                        continue;
                    };
                    if tx.send(event).is_err() {
                        let _ = child.kill();
                        let _ = child.wait();
                        return;
                    }
                }
            }
        }
        let _ = tx.send(WatchEvent::Dead);
        let _ = child.kill();
        let _ = child.wait();
    });
    rx
}

/// Put a PNG file on the clipboard.
pub fn write_image(path: &Path) -> bool {
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    Command::new("wl-copy")
        .args(["--type", "image/png"])
        .stdin(file)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

pub fn write_clipboard(text: &str) -> bool {
    let child = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    match child {
        Ok(mut child) => {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_wins_over_images() {
        assert!(has_text(&["text/plain;charset=utf-8", "image/png"]));
        assert_eq!(image_type(&["text/plain", "image/png"]), None);
        assert_eq!(
            image_type(&["text/html", "image/jpeg", "image/png"]),
            Some("image/png")
        );
        assert_eq!(image_type(&["image/bmp"]), Some("image/bmp"));
        assert_eq!(image_type(&["text/html"]), None);
        assert!(is_secret(&["x-kde-passwordManagerHint", "text/plain"]));
    }

    #[test]
    fn image_file_only_for_a_single_image_file() {
        let dir = std::env::temp_dir().join(format!("pastel-img-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("a.png");
        // Smallest valid PNG: one transparent pixel
        let bytes: &[u8] = &[
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, 0x49, 0x48, 0x44, 0x52,
            0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4, 0x89, 0, 0, 0, 0x0d, 0x49,
            0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0, 1, 0, 0, 5, 0, 1, 0x0d, 0x0a, 0x2d, 0xb4, 0, 0,
            0, 0, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];
        std::fs::write(&png, bytes).unwrap();
        std::fs::write(dir.join("a.txt"), "hi").unwrap();
        let uri = |name: &str| gio::File::for_path(dir.join(name)).uri().to_string();

        assert_eq!(image_file(png.to_str().unwrap()), Some(png.clone()));
        assert_eq!(image_file(&uri("a.png")), Some(png.clone()));
        assert_eq!(image_file(&uri("a.txt")), None);
        assert_eq!(
            image_file(&format!("{}\n{}", uri("a.png"), uri("a.txt"))),
            None
        );
        assert_eq!(image_file("see /tmp/a.png"), None);
        assert_eq!(image_file("/no/such/file.png"), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
