use crate::limitador::{Limitador, Prioridad};
use crate::modelos::{Cancion, FuentesError, ListaCanciones, ResultadoResolucion};
use crate::ytdlp::{proxy_for, run_json, RunJsonOptions, YtDlpSettings};
use std::time::Duration;
use url::Url;

pub fn valida_url_generica(input: &str) -> bool {
    if let Ok(u) = Url::parse(input) {
        u.scheme() == "http" || u.scheme() == "https"
    } else {
        false
    }
}

pub fn json_to_song_generica(entry: &serde_json::Value, fallback_url: &str) -> Cancion {
    let source = entry["extractor_key"]
        .as_str()
        .or_else(|| entry["ie_key"].as_str())
        .or_else(|| entry["extractor"].as_str())
        .unwrap_or("generic")
        .to_lowercase();

    let id = entry["id"]
        .as_str()
        .or_else(|| entry["url"].as_str())
        .unwrap_or("")
        .to_string();

    let name = entry["title"]
        .as_str()
        .unwrap_or(if !id.is_empty() { &id } else { fallback_url })
        .to_string();

    let song_url = entry["webpage_url"]
        .as_str()
        .or_else(|| entry["url"].as_str())
        .unwrap_or(fallback_url)
        .to_string();

    let is_live = entry["is_live"].as_bool().unwrap_or(false);
    let duration = if is_live {
        0
    } else {
        entry["duration"].as_u64().unwrap_or(0)
    };

    let thumbnail = entry["thumbnail"]
        .as_str()
        .map(ToString::to_string)
        .or_else(|| {
            entry["thumbnails"]
                .as_array()
                .and_then(|arr| arr.last())
                .and_then(|t| t["url"].as_str())
                .map(ToString::to_string)
        });

    let uploader = entry["uploader"]
        .as_str()
        .or_else(|| entry["channel"].as_str())
        .unwrap_or("")
        .to_string();

    let author_url = entry["uploader_url"]
        .as_str()
        .or_else(|| entry["channel_url"].as_str())
        .map(ToString::to_string);

    let age_restricted = entry["age_limit"].as_u64().unwrap_or(0) >= 18;

    Cancion {
        fuente: source,
        id,
        nombre: name,
        artista: uploader,
        url: song_url,
        stream_url: entry["url"].as_str().map(ToString::to_string),
        es_directo: is_live,
        duracion: duration,
        miniatura: thumbnail,
        autor_url: author_url,
        edad_restringida: age_restricted,
        es_letra: false,
    }
}

pub async fn resolver_generica(
    binario: &str,
    url: &str,
    guild_id: Option<&str>,
    settings: &YtDlpSettings,
    proxies: &[String],
    limiter: &Limitador,
    max_playlist_size: usize,
) -> Result<ResultadoResolucion, FuentesError> {
    if !valida_url_generica(url) {
        return Err(FuentesError::UrlNoSoportada);
    }

    let proxy = proxy_for(guild_id, proxies);
    let extra = vec![
        "--flat-playlist".to_string(),
        "--playlist-end".to_string(),
        max_playlist_size.to_string(),
        "-f".to_string(),
        "bestaudio/best".to_string(),
    ];
    let base = crate::ytdlp::base_args(settings, proxy.as_deref(), None);

    let val = run_json(
        binario,
        url,
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

    if let Some(entries) = val["entries"].as_array() {
        let mut songs = Vec::new();
        for entry in entries {
            if entry["webpage_url"].is_string() || entry["url"].is_string() {
                songs.push(json_to_song_generica(entry, url));
            }
        }
        if songs.is_empty() {
            return Err(FuentesError::ListaVacia);
        }

        let id = val["id"].as_str().unwrap_or(url).to_string();
        let name = val["title"].as_str().unwrap_or(url).to_string();
        let list_url = val["webpage_url"].as_str().unwrap_or(url).to_string();
        let thumb = val["thumbnail"]
            .as_str()
            .map(ToString::to_string)
            .or_else(|| songs.first().and_then(|s| s.miniatura.clone()));

        Ok(ResultadoResolucion::Lista(ListaCanciones {
            fuente: val["extractor_key"]
                .as_str()
                .unwrap_or("generic")
                .to_lowercase(),
            id,
            nombre: name,
            url: list_url,
            miniatura: thumb,
            canciones: songs,
            truncada: false,
        }))
    } else {
        Ok(ResultadoResolucion::Cancion(json_to_song_generica(
            &val, url,
        )))
    }
}
