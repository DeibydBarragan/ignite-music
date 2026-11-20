use crate::genericas::resolver_generica;
use crate::limitador::Limitador;
use crate::modelos::{Cancion, FuentesError, ResultadoResolucion};
use crate::spotify::SpotifyClient;
use crate::youtube::resolver_youtube;
use crate::ytdlp::YtDlpSettings;
use serde::{Deserialize, Serialize};
use url::Url;

pub const FALLBACK_COVER: &str =
    "https://images.unsplash.com/photo-1470225620780-dba8ba36b745?w=300&q=80";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSnapshot {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album_art: String,
    pub cover_url: Option<String>,
    pub duration: u64,
    pub formatted_duration: String,
    pub source: String,
    pub url: String,
    pub category: String,
    pub is_live: bool,
    pub is_lyric: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultadoPlaylist {
    pub kind: String,
    pub name: String,
    pub tracks: Vec<TrackSnapshot>,
    pub truncated: bool,
    pub source: String,
}

pub fn format_duration(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

pub fn snapshot_track(song: &Cancion) -> TrackSnapshot {
    let source_normalized = if song.fuente == "spotify" {
        "spotify"
    } else {
        "youtube"
    };

    TrackSnapshot {
        id: if !song.id.is_empty() {
            song.id.clone()
        } else {
            song.url.clone()
        },
        title: if !song.nombre.is_empty() {
            song.nombre.clone()
        } else {
            "Unknown Title".to_string()
        },
        artist: if !song.artista.is_empty() {
            song.artista.clone()
        } else {
            "Unknown Artist".to_string()
        },
        album_art: song
            .miniatura
            .clone()
            .unwrap_or_else(|| FALLBACK_COVER.to_string()),
        cover_url: song.miniatura.clone(),
        duration: song.duracion,
        formatted_duration: if song.duracion == 0 {
            "00:00".into()
        } else {
            format_duration(song.duracion)
        },
        source: source_normalized.to_string(),
        url: song.url.clone(),
        category: "hits".to_string(),
        is_live: song.es_directo,
        is_lyric: song.es_letra,
    }
}

pub fn detect_source(url: &str) -> &'static str {
    let lower = url.to_lowercase();
    if lower.contains("spotify.com") {
        "spotify"
    } else if lower.contains("youtube.com") || lower.contains("youtu.be") {
        "youtube"
    } else {
        "generic"
    }
}

pub fn derive_name(url: &str, custom_name: Option<&str>) -> String {
    if let Some(wanted) = custom_name {
        let trimmed = wanted.trim();
        if !trimmed.is_empty() {
            return trimmed.chars().take(60).collect();
        }
    }

    if let Ok(parsed) = Url::parse(url.trim()) {
        let parts: Vec<&str> = parsed.path().split('/').filter(|s| !s.is_empty()).collect();
        if let Some(last) = parts.last() {
            if let Ok(decoded) = percent_encoding::percent_decode_str(last).decode_utf8() {
                let cleaned = decoded
                    .chars()
                    .map(|c| if c == '-' || c == '_' { ' ' } else { c })
                    .collect::<String>();
                let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
                if !collapsed.is_empty() {
                    return collapsed.chars().take(60).collect();
                }
            }
        }
    }

    "Nueva playlist".to_string()
}

#[allow(clippy::too_many_arguments)]
pub async fn resolve_playlist(
    url: &str,
    custom_name: Option<&str>,
    max_size: Option<usize>,
    spotify_client: Option<&SpotifyClient>,
    binario: &str,
    guild_id: Option<&str>,
    settings: &YtDlpSettings,
    proxies: &[String],
    limiter: &Limitador,
) -> Result<ResultadoPlaylist, FuentesError> {
    let clean = url.trim();
    if clean.is_empty() {
        return Err(FuentesError::UrlNoSoportada);
    }

    let limit = max_size.unwrap_or(200).max(1);
    let source = detect_source(clean);

    let res = match source {
        "spotify" => {
            let client = spotify_client.ok_or_else(|| {
                FuentesError::Spotify("Spotify client not configured".to_string())
            })?;
            client.resolve(clean).await?
        }
        "youtube" => {
            resolver_youtube(binario, clean, guild_id, settings, proxies, limiter, limit).await?
        }
        _ => {
            resolver_generica(
                binario,
                clean,
                guild_id,
                settings,
                proxies,
                limiter,
                limit.saturating_add(1),
            )
            .await?
        }
    };

    match res {
        ResultadoResolucion::Lista(l) => {
            let total = l.canciones.len();
            let truncated = l.truncada || total > limit || (source == "youtube" && total == limit);
            let tracks: Vec<TrackSnapshot> = l
                .canciones
                .into_iter()
                .take(limit)
                .map(|c| snapshot_track(&c))
                .collect();
            let name = nombre_resuelto(clean, custom_name, l.nombre);
            Ok(ResultadoPlaylist {
                kind: "playlist".to_string(),
                name,
                tracks,
                truncated,
                source: source.to_string(),
            })
        }
        ResultadoResolucion::Cancion(c) => {
            let name = nombre_resuelto(clean, custom_name, c.nombre.clone());
            Ok(ResultadoPlaylist {
                kind: "track".to_string(),
                name,
                tracks: vec![snapshot_track(&c)],
                truncated: false,
                source: source.to_string(),
            })
        }
    }
}

fn nombre_resuelto(url: &str, custom_name: Option<&str>, extraido: String) -> String {
    if let Some(nombre) = custom_name.filter(|s| !s.trim().is_empty()) {
        derive_name(url, Some(nombre))
    } else if extraido.is_empty() {
        derive_name(url, None)
    } else {
        extraido
    }
}
