//! The `Note` type and date helpers.

use chrono::{Local, TimeZone};
use uuid::Uuid;

/// Title given to freshly created notes.
pub const DEFAULT_TITLE: &str = "Untitled Note";

/// A single note. Timestamps are Unix seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub created_at: i64,
    pub modified_at: i64,
}

/// Current time as Unix seconds.
pub fn now() -> i64 {
    Local::now().timestamp()
}

impl Note {
    /// Create a new note with a freshly generated UUID.
    pub fn new(title: &str, content: &str) -> Self {
        let now = now();
        Note {
            id: Uuid::new_v4().to_string(),
            title: title.to_owned(),
            content: content.to_owned(),
            created_at: now,
            modified_at: now,
        }
    }
}

/// Local date such as `2026-11-07`, for list rows.
pub fn date_string(timestamp: i64) -> String {
    if timestamp == 0 {
        return "Unknown".to_owned();
    }
    match Local.timestamp_opt(timestamp, 0).single() {
        Some(time) => time.format("%Y-%m-%d").to_string(),
        None => "Invalid date".to_owned(),
    }
}
