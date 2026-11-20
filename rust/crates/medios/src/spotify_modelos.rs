use crate::modelos::Cancion;
use regex::Regex;
use serde::Deserialize;
use std::sync::LazyLock;

static RE_SPOTIFY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:open\.spotify\.com/|spotify:)(track|album|playlist)[/:]([a-zA-Z0-9]+)")
        .unwrap()
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotifyUrlType {
    Track(String),
    Album(String),
    Playlist(String),
}

pub fn parse_spotify_url(input: &str) -> Option<SpotifyUrlType> {
    let caps = RE_SPOTIFY.captures(input)?;
    let kind = caps.get(1)?.as_str().to_lowercase();
    let id = caps.get(2)?.as_str().to_string();
    match kind.as_str() {
        "track" => Some(SpotifyUrlType::Track(id)),
        "album" => Some(SpotifyUrlType::Album(id)),
        "playlist" => Some(SpotifyUrlType::Playlist(id)),
        _ => None,
    }
}

pub fn create_search_query(artista: &str, nombre: &str) -> String {
    if !artista.is_empty() {
        format!("{artista} - {nombre}")
    } else {
        nombre.to_string()
    }
}

#[derive(Deserialize)]
pub(crate) struct TokenResponse {
    pub(crate) access_token: String,
    pub(crate) expires_in: u64,
}

pub fn spotify_track_to_song(track: &serde_json::Value) -> Cancion {
    let id = track["id"].as_str().unwrap_or("").to_string();
    let name = track["name"].as_str().unwrap_or("").to_string();
    let artists = track["artists"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a["name"].as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();

    let duration_ms = track["duration_ms"].as_u64().unwrap_or(0);
    let thumbnail = track["album"]["images"][0]["url"]
        .as_str()
        .map(ToString::to_string);

    Cancion {
        fuente: "spotify".to_string(),
        id: id.clone(),
        nombre: name,
        artista: artists,
        url: format!("https://open.spotify.com/track/{id}"),
        stream_url: None,
        es_directo: false,
        duracion: duration_ms / 1000,
        miniatura: thumbnail,
        autor_url: None,
        edad_restringida: false,
        es_letra: false,
    }
}
