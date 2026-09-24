//! Small text/time formatting helpers.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ts() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Single-line preview of a clip for the list rows.
pub fn preview_text(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| match c {
            '\n' | '\r' => '¶',
            '\t' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let flat = flat.trim();
    const PREVIEW_CHARS: usize = 200;
    if flat.chars().count() > PREVIEW_CHARS {
        let cut: String = flat.chars().take(PREVIEW_CHARS).collect();
        format!("{} …", cut)
    } else {
        flat.to_string()
    }
}

pub fn rel_time(ts: f64) -> String {
    let age = (now_ts() - ts).max(0.0) as u64;
    if age < 45 {
        "just now".into()
    } else if age < 3600 {
        format!("{}m ago", age / 60)
    } else if age < 86_400 {
        format!("{}h ago", age / 3600)
    } else if age < 30 * 86_400 {
        format!("{}d ago", age / 86_400)
    } else {
        "long ago".into()
    }
}
