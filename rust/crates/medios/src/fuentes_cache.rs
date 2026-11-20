use crate::limitador::{Limitador, Prioridad};
use crate::modelos::{Cancion, FuentesError};
use crate::youtube::{stream_ttl, AUDIO_FORMAT};
use crate::youtube_busqueda::buscar_cancion;
use crate::ytdlp::{base_args, proxy_for, run_json, RunJsonOptions, YtDlpSettings};
use nucleo::CacheTtl;
use std::time::Duration;

pub const CACHE_BUSQUEDA_TTL_MS: u64 = 30 * 60_000;
pub const CACHE_BUSQUEDA_MAX_SIZE: usize = 1000;
pub const CACHE_INFOS_TTL_MS: u64 = 3_600_000;
pub const CACHE_INFOS_MAX_SIZE: usize = 500;
pub const CACHE_LETRAS_TTL_MS: u64 = 24 * 3_600_000;
pub const CACHE_LETRAS_MAX_SIZE: usize = 5000;

#[derive(Clone)]
pub struct FuentesCache {
    pub searches: CacheTtl<Option<Cancion>>,
    pub infos: CacheTtl<serde_json::Value>,
    pub lyrics: CacheTtl<bool>,
}

impl Default for FuentesCache {
    fn default() -> Self {
        Self::new()
    }
}

impl FuentesCache {
    pub fn new() -> Self {
        Self {
            searches: CacheTtl::new(Some(CACHE_BUSQUEDA_TTL_MS), Some(CACHE_BUSQUEDA_MAX_SIZE)),
            infos: CacheTtl::new(Some(CACHE_INFOS_TTL_MS), Some(CACHE_INFOS_MAX_SIZE)),
            lyrics: CacheTtl::new(Some(CACHE_LETRAS_TTL_MS), Some(CACHE_LETRAS_MAX_SIZE)),
        }
    }

    pub fn info_key(proxy: Option<&str>, url: &str) -> String {
        format!("{}|{}", proxy.unwrap_or("direct"), url)
    }

    pub fn search_key(proxy: Option<&str>, query: &str) -> String {
        format!("{}|{}", proxy.unwrap_or("direct"), query.to_lowercase())
    }

    pub fn remember_info(&self, proxy: Option<&str>, page_url: &str, info: &serde_json::Value) {
        let has_url = info["url"].as_str().map(|s| !s.is_empty()).unwrap_or(false);
        let has_format = info["format_id"]
            .as_str()
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        let no_entries = info["entries"].is_null();
        if has_url && has_format && no_entries {
            let key = Self::info_key(proxy, page_url);
            let stream_url = info["url"].as_str().unwrap_or("");
            let ttl = stream_ttl(stream_url);
            self.infos.set(&key, info.clone(), Some(ttl));
        }
    }

    pub fn is_lyric_song(&self, url: &str) -> bool {
        self.lyrics.get(url).unwrap_or(false)
    }

    pub fn set_lyric_song(&self, url: &str, is_lyric: bool) {
        self.lyrics.set(url, is_lyric, Some(CACHE_LETRAS_TTL_MS));
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn stream_info(
    binario: &str,
    song_url: &str,
    guild_id: Option<&str>,
    settings: &YtDlpSettings,
    proxies: &[String],
    limiter: &Limitador,
    prioridad: Prioridad,
    cache: &FuentesCache,
) -> Result<serde_json::Value, FuentesError> {
    if song_url.trim().is_empty() {
        return Err(FuentesError::SinStreamUrl(
            "Cannot get a stream without a song URL".to_string(),
        ));
    }

    let proxy = proxy_for(guild_id, proxies);
    let key = FuentesCache::info_key(proxy.as_deref(), song_url);

    let binario_s = binario.to_string();
    let song_url_s = song_url.to_string();
    let limiter_c = limiter.clone();
    let base = base_args(settings, proxy.as_deref(), None);
    let timeout = Duration::from_millis(settings.timeout_ms);
    let proxy_c = proxy.clone();
    let cache_c = cache.clone();

    let info = cache
        .infos
        .wrap(
            &key,
            || async move {
                let extra = vec![
                    "--no-playlist".to_string(),
                    "-f".to_string(),
                    AUDIO_FORMAT.to_string(),
                ];
                let res = run_json(
                    &binario_s,
                    &song_url_s,
                    &limiter_c,
                    RunJsonOptions {
                        base_args: &base,
                        extra_args: &extra,
                        prioridad,
                        timeout,
                        queue_timeout: timeout,
                    },
                )
                .await
                .map_err(|e| FuentesError::YtDlp(e.mensaje))?;

                if res["url"].as_str().map(|s| s.is_empty()).unwrap_or(true) {
                    return Err(FuentesError::SinStreamUrl(song_url_s));
                }

                Ok(res)
            },
            Some(CACHE_INFOS_TTL_MS),
        )
        .await?;

    cache_c.remember_info(proxy_c.as_deref(), song_url, &info);
    Ok(info)
}

pub async fn prefetch(
    binario: &str,
    cancion: &Cancion,
    guild_id: Option<&str>,
    settings: &YtDlpSettings,
    proxies: &[String],
    limiter: &Limitador,
    cache: &FuentesCache,
) -> Result<serde_json::Value, FuentesError> {
    stream_info(
        binario,
        &cancion.url,
        guild_id,
        settings,
        proxies,
        limiter,
        Prioridad::Baja,
        cache,
    )
    .await
}

pub async fn prefetch_query(
    binario: &str,
    query: &str,
    guild_id: Option<&str>,
    settings: &YtDlpSettings,
    proxies: &[String],
    limiter: &Limitador,
    cache: &FuentesCache,
) -> Result<Option<Cancion>, FuentesError> {
    let song = buscar_cancion(
        binario,
        query,
        guild_id,
        settings,
        proxies,
        limiter,
        Prioridad::Baja,
        None,
        Some(cache),
    )
    .await?;

    if let Some(ref s) = song {
        if let Some(ref stream_url) = s.stream_url {
            let proxy = proxy_for(guild_id, proxies);
            let dummy = serde_json::json!({
                "id": s.id,
                "url": stream_url,
                "format_id": "bestaudio",
            });
            cache.remember_info(proxy.as_deref(), &s.url, &dummy);
        }
    }

    Ok(song)
}
