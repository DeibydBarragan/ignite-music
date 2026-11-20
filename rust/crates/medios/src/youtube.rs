use crate::limitador::{Limitador, Prioridad};
use crate::modelos::{Cancion, FuentesError, ListaCanciones, ResultadoResolucion};
use crate::youtube_titulo::{is_lyric_title, split_youtube_title};
use crate::ytdlp::{proxy_for, run_json, RunJsonOptions, YtDlpSettings};
use std::time::Duration;

pub use crate::youtube_titulo::{stream_ttl, stream_ttl_at};
pub use crate::youtube_url::{
    is_playlist_id, is_video_id, parse_youtube_url, thumbnail_for, watch_url, YouTubeUrlType,
};

pub const AUDIO_FORMAT: &str = "bestaudio[acodec=opus]/bestaudio/best";
pub const RELATED_MIN_SECONDS: u64 = 60;
pub const RELATED_MAX_SECONDS: u64 = 900;

pub fn youtube_info(info: &serde_json::Value) -> Cancion {
    let id = info["id"].as_str().unwrap_or("").to_string();
    let is_live = info["is_live"].as_bool().unwrap_or(false)
        || info["live_status"].as_str() == Some("is_live");
    let uploader = info["uploader"]
        .as_str()
        .or_else(|| info["channel"].as_str());
    let raw_title = info["title"]
        .as_str()
        .or_else(|| info["fulltitle"].as_str())
        .unwrap_or(id.as_str());
    let (title, artist) = split_youtube_title(raw_title, uploader);
    let is_lyric = is_lyric_title(raw_title);
    let valid_id = is_video_id(&id);
    let url = if valid_id {
        watch_url(&id)
    } else {
        info["webpage_url"]
            .as_str()
            .or_else(|| info["url"].as_str())
            .unwrap_or("")
            .to_string()
    };
    let thumbnail = if valid_id {
        Some(thumbnail_for(&id))
    } else {
        info["thumbnail"].as_str().map(ToString::to_string)
    };
    let duration = if is_live {
        0
    } else {
        info["duration"].as_u64().unwrap_or(0)
    };
    let age_limit = info["age_limit"].as_u64().unwrap_or(0);

    Cancion {
        fuente: "youtube".to_string(),
        id,
        nombre: title,
        artista: if !artist.is_empty() {
            artist
        } else {
            uploader.unwrap_or("").to_string()
        },
        url,
        stream_url: info["url"].as_str().map(ToString::to_string),
        es_directo: is_live,
        duracion: duration,
        miniatura: thumbnail,
        autor_url: info["uploader_url"]
            .as_str()
            .or_else(|| info["channel_url"].as_str())
            .map(ToString::to_string),
        edad_restringida: age_limit >= 18,
        es_letra: is_lyric,
    }
}

pub fn is_playable_related(entry: &serde_json::Value, current_id: &str) -> bool {
    let id = entry["id"].as_str().unwrap_or("");
    if id.is_empty() || id == current_id || !is_video_id(id) {
        return false;
    }
    if let Some(dur) = entry["duration"].as_u64() {
        (RELATED_MIN_SECONDS..=RELATED_MAX_SECONDS).contains(&dur)
    } else {
        true
    }
}

async fn run_yt(
    binario: &str,
    target: &str,
    extra: &[String],
    proxy: Option<&str>,
    settings: &YtDlpSettings,
    limiter: &Limitador,
) -> Result<serde_json::Value, FuentesError> {
    let base = crate::ytdlp::base_args(settings, proxy, None);
    run_json(
        binario,
        target,
        limiter,
        RunJsonOptions {
            base_args: &base,
            extra_args: extra,
            prioridad: Prioridad::Alta,
            timeout: Duration::from_millis(settings.timeout_ms),
            queue_timeout: Duration::from_millis(settings.timeout_ms),
        },
    )
    .await
    .map_err(|e| FuentesError::YtDlp(e.mensaje))
}

pub async fn resolver_youtube(
    binario: &str,
    url: &str,
    guild_id: Option<&str>,
    settings: &YtDlpSettings,
    proxies: &[String],
    limiter: &Limitador,
    max_playlist_size: usize,
) -> Result<ResultadoResolucion, FuentesError> {
    let parsed = parse_youtube_url(url).ok_or(FuentesError::UrlNoSoportada)?;
    let proxy = proxy_for(guild_id, proxies);

    match parsed {
        YouTubeUrlType::Playlist { id } => {
            let target = format!("https://www.youtube.com/playlist?list={id}");
            let extra = vec![
                "--flat-playlist".to_string(),
                "--playlist-end".to_string(),
                max_playlist_size.to_string(),
            ];
            let val = run_yt(
                binario,
                &target,
                &extra,
                proxy.as_deref(),
                settings,
                limiter,
            )
            .await?;

            let entries = val["entries"].as_array().ok_or(FuentesError::ListaVacia)?;
            let mut songs = Vec::new();
            for entry in entries {
                let eid = entry["id"].as_str().unwrap_or("");
                let etitle = entry["title"].as_str().unwrap_or("");
                if is_video_id(eid) && etitle != "[Private video]" && etitle != "[Deleted video]" {
                    songs.push(youtube_info(entry));
                }
            }
            if songs.is_empty() {
                return Err(FuentesError::ListaVacia);
            }
            let title = val["title"]
                .as_str()
                .unwrap_or("YouTube playlist")
                .to_string();
            let thumb = songs.first().and_then(|s| s.miniatura.clone());
            Ok(ResultadoResolucion::Lista(ListaCanciones {
                fuente: "youtube".to_string(),
                id,
                nombre: title,
                url: target,
                miniatura: thumb,
                canciones: songs,
                truncada: false,
            }))
        }
        YouTubeUrlType::Video { id } => {
            let target = watch_url(&id);
            let extra = vec![
                "--no-playlist".to_string(),
                "-f".to_string(),
                AUDIO_FORMAT.to_string(),
            ];
            let val = run_yt(
                binario,
                &target,
                &extra,
                proxy.as_deref(),
                settings,
                limiter,
            )
            .await?;

            let song = youtube_info(&val);
            if song.stream_url.is_none() {
                return Err(FuentesError::SinStreamUrl(song.nombre));
            }
            Ok(ResultadoResolucion::Cancion(song))
        }
    }
}
