use crate::limitador::{Limitador, Prioridad};
use crate::modelos::{Cancion, FuentesError, ListaEncontrada};
use crate::youtube::{is_playable_related, is_video_id, watch_url, youtube_info};
use crate::ytdlp::{proxy_for, run_json, RunJsonOptions, YtDlpSettings};
use std::time::Duration;

pub async fn buscar_listas(
    binario: &str,
    query: &str,
    limit: usize,
    guild_id: Option<&str>,
    settings: &YtDlpSettings,
    proxies: &[String],
    limiter: &Limitador,
) -> Result<Vec<ListaEncontrada>, FuentesError> {
    let proxy = proxy_for(guild_id, proxies);
    let encoded = urlencoding_simple(query);
    let target =
        format!("https://www.youtube.com/results?search_query={encoded}&sp=EgIQAw%253D%253D");
    let extra = vec!["--flat-playlist".to_string()];
    let base = crate::ytdlp::base_args(settings, proxy.as_deref(), None);

    let val = run_json(
        binario,
        &target,
        limiter,
        RunJsonOptions {
            base_args: &base,
            extra_args: &extra,
            prioridad: Prioridad::Alta,
            timeout: Duration::from_millis(settings.timeout_ms),
            queue_timeout: Duration::from_millis(settings.timeout_ms),
        },
    )
    .await
    .map_err(|e| FuentesError::YtDlp(e.mensaje))?;

    Ok(val["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let texto = |field: &str| entry[field].as_str().filter(|s| !s.is_empty());
            let id = texto("id")?;
            let miniatura = entry["thumbnails"][0]["url"]
                .as_str()
                .filter(|s| !s.is_empty())
                .or_else(|| texto("thumbnail"))
                .map(str::to_owned);
            let cantidad_pistas = ["playlist_count", "n_entries"]
                .into_iter()
                .find_map(|field| {
                    entry[field]
                        .as_u64()
                        .or_else(|| texto(field)?.parse().ok())
                        .filter(|&n| n > 0)
                })
                .unwrap_or(0);
            Some(ListaEncontrada {
                id: id.into(),
                nombre: texto("title").unwrap_or("Unknown Playlist").into(),
                autor: texto("uploader")
                    .or_else(|| texto("channel"))
                    .unwrap_or("YouTube")
                    .into(),
                url: format!("https://www.youtube.com/playlist?list={id}"),
                miniatura,
                cantidad_pistas,
            })
        })
        .take(limit)
        .collect())
}

pub async fn relacionados(
    binario: &str,
    video_id: &str,
    guild_id: Option<&str>,
    settings: &YtDlpSettings,
    proxies: &[String],
    limiter: &Limitador,
) -> Result<Vec<Cancion>, FuentesError> {
    if !is_video_id(video_id) {
        return Ok(Vec::new());
    }

    let proxy = proxy_for(guild_id, proxies);
    let target = format!("{}&list=RD{}", watch_url(video_id), video_id);
    let extra = vec![
        "--flat-playlist".to_string(),
        "--playlist-end".to_string(),
        "15".to_string(),
    ];
    let base = crate::ytdlp::base_args(settings, proxy.as_deref(), None);

    let val = run_json(
        binario,
        &target,
        limiter,
        RunJsonOptions {
            base_args: &base,
            extra_args: &extra,
            prioridad: Prioridad::Baja,
            timeout: Duration::from_millis(settings.timeout_ms),
            queue_timeout: Duration::from_millis(settings.timeout_ms),
        },
    )
    .await
    .map_err(|e| FuentesError::YtDlp(e.mensaje))?;

    let mut songs = Vec::new();
    if let Some(entries) = val["entries"].as_array() {
        for entry in entries {
            if is_playable_related(entry, video_id) {
                songs.push(youtube_info(entry));
            }
        }
    }
    Ok(songs)
}

pub fn urlencoding_simple(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}
