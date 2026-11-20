//! Modelos y utilidades para listas de reproducción de usuario (Spec S-30).

use crate::api::formas::FALLBACK_COVER;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_USER_PLAYLISTS: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserPlaylist {
    pub id: Option<String>,
    pub user_id: String,
    pub name: String,
    pub source: String,
    pub url: String,
    pub cover: String,
    pub tracks: Vec<Value>,
    pub synced_at: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UserPlaylistInput {
    pub user_id: String,
    pub name: Option<String>,
    pub url: String,
    pub source: Option<String>,
    pub cover: Option<String>,
    pub tracks: Vec<Value>,
    pub synced_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResultadoUpsertPlaylist {
    pub playlist: UserPlaylist,
    pub created: bool,
}

pub fn detect_source(url: &str) -> String {
    let lower = url.to_lowercase();
    if lower.contains("spotify.com") {
        "spotify".into()
    } else if lower.contains("youtube.com") || lower.contains("youtu.be") {
        "youtube".into()
    } else {
        "generic".into()
    }
}

pub fn decodificar_segmento(raw: &str) -> String {
    let mut bytes = Vec::new();
    let mut chars = raw.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            if let (Some(h1), Some(h2)) = (chars.next(), chars.next()) {
                if let Ok(val) =
                    u8::from_str_radix(std::str::from_utf8(&[h1, h2]).unwrap_or(""), 16)
                {
                    bytes.push(val);
                    continue;
                }
            }
        }
        bytes.push(b);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn derive_name(url: &str, custom_name: Option<&str>) -> String {
    if let Some(wanted) = custom_name.map(str::trim).filter(|s| !s.is_empty()) {
        return wanted.chars().take(60).collect();
    }
    if let Ok(parsed) = url::Url::parse(url.trim()) {
        let parts: Vec<&str> = parsed
            .path_segments()
            .map(|s| s.collect())
            .unwrap_or_default();
        if let Some(last) = parts.into_iter().rfind(|s| !s.is_empty()) {
            let decoded = decodificar_segmento(last);
            let clean: String = decoded.replace(['-', '_'], " ").chars().take(60).collect();
            if !clean.trim().is_empty() {
                return clean;
            }
        }
    }
    "Nueva playlist".into()
}

pub fn map_row(row: &Value) -> UserPlaylist {
    let tracks = row
        .get("tracks")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let cover = row
        .get("cover")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            tracks
                .first()
                .and_then(|t| t.get("albumArt"))
                .and_then(Value::as_str)
        })
        .unwrap_or(FALLBACK_COVER)
        .to_string();

    UserPlaylist {
        id: row.get("id").and_then(Value::as_str).map(String::from),
        user_id: row
            .get("user_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        name: row
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Playlist sin nombre")
            .into(),
        source: row
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("spotify")
            .into(),
        url: row
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        cover,
        tracks,
        synced_at: row
            .get("synced_at")
            .and_then(Value::as_str)
            .map(String::from),
        created_at: row
            .get("created_at")
            .and_then(Value::as_str)
            .map(String::from),
    }
}
