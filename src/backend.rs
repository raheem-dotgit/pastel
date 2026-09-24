//! Clipboard backend: event-driven Wayland capture via `wl-paste --watch`.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;

/// One clipboard event from the background watcher.
pub enum WatchEvent {
    Clip(String),
    Dead,
}

/// Stream clipboard changes from `wl-paste --watch`. Each change is printed
/// followed by a NUL byte, which separates the clips on the pipe.
pub fn spawn_wayland_watcher() -> Receiver<WatchEvent> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let child = Command::new("wl-paste")
            .args(["--no-newline", "--watch", "sh", "-c", "cat; printf '\\000'"])
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

        let mut buf = [0u8; 8192];
        let mut acc: Vec<u8> = Vec::new();
        loop {
            match stdout.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    acc.extend_from_slice(&buf[..n]);
                    while let Some(pos) = acc.iter().position(|&b| b == 0) {
                        let text = String::from_utf8_lossy(&acc[..pos]).into_owned();
                        acc.drain(..=pos);
                        if clipboard_is_secret() {
                            continue;
                        }
                        if tx.send(WatchEvent::Clip(text)).is_err() {
                            let _ = child.kill();
                            let _ = child.wait();
                            return;
                        }
                    }
                    // Drop the buffer if a separator never arrives
                    if acc.len() > 10_000_000 {
                        acc.clear();
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

/// True when a password manager (KeePassXC etc.) marked the clip as secret.
pub fn clipboard_is_secret() -> bool {
    Command::new("wl-paste")
        .arg("--list-types")
        .output()
        .map(|o| {
            o.status.success() && String::from_utf8_lossy(&o.stdout).contains("passwordManagerHint")
        })
        .unwrap_or(false)
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
