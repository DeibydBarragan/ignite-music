use crate::errores::{is_bot_check, YoutubeSearchErrorState};
use crate::fuentes_cache::{FuentesCache, CACHE_BUSQUEDA_TTL_MS};
use crate::limitador::{Limitador, Prioridad};
use crate::modelos::{Cancion, FuentesError};
use crate::youtube::{is_video_id, youtube_info, AUDIO_FORMAT};
use crate::ytdlp::{proxy_for, run_json, RunJsonOptions, YtDlpSettings};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub use crate::youtube_relacionados::{buscar_listas, relacionados, urlencoding_simple};

#[allow(clippy::too_many_arguments)]
async fn buscar_cancion_uncached(
    binario: &str,
    query: &str,
    proxy: Option<&str>,
    settings: &YtDlpSettings,
    limiter: &Limitador,
    prioridad: Prioridad,
    error_state: Option<&Arc<Mutex<YoutubeSearchErrorState>>>,
) -> Result<Option<Cancion>, FuentesError> {
    let target = format!("ytsearch1:{query}");
    let extra = vec!["-f".to_string(), AUDIO_FORMAT.to_string()];
    let base = crate::ytdlp::base_args(settings, proxy, None);

    let opts = RunJsonOptions {
        base_args: &base,
        extra_args: &extra,
        prioridad,
        timeout: Duration::from_millis(settings.timeout_ms),
        queue_timeout: Duration::from_millis(settings.timeout_ms),
    };

    let first = run_json(binario, &target, limiter, opts.clone()).await;
    let res = match first {
        Ok(val) => Ok(val),
        Err(err) => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            if let Some(state_arc) = error_state {
                let mut state = state_arc.lock().unwrap();
                state.last_search_error = Some(err.mensaje.clone());
                state.last_search_error_at = now;
            }

            let text = format!("{} {}", err.mensaje, err.stderr);
            if is_bot_check(&text) {
                tokio::time::sleep(Duration::from_secs(8)).await;
                match run_json(binario, &target, limiter, opts).await {
                    Ok(val) => Ok(val),
                    Err(retry_err) => {
                        if let Some(state_arc) = error_state {
                            let mut state = state_arc.lock().unwrap();
                            state.last_search_error = Some(retry_err.mensaje.clone());
                            state.last_search_error_at = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis()
                                as u64;
                        }
                        return Err(FuentesError::YtDlp(retry_err.mensaje));
                    }
                }
            } else {
                return Err(FuentesError::YtDlp(err.mensaje));
            }
        }
    }?;

    let info_obj = if let Some(entries) = res["entries"].as_array() {
        entries.iter().find(|e| !e.is_null()).unwrap_or(&res)
    } else {
        &res
    };

    let id = info_obj["id"].as_str().unwrap_or("");
    if id.is_empty() {
        return Ok(None);
    }

    Ok(Some(youtube_info(info_obj)))
}

#[allow(clippy::too_many_arguments)]
pub async fn buscar_cancion(
    binario: &str,
    query: &str,
    guild_id: Option<&str>,
    settings: &YtDlpSettings,
    proxies: &[String],
    limiter: &Limitador,
    prioridad: Prioridad,
    error_state: Option<&Arc<Mutex<YoutubeSearchErrorState>>>,
    cache: Option<&FuentesCache>,
) -> Result<Option<Cancion>, FuentesError> {
    let proxy = proxy_for(guild_id, proxies);

    if let Some(c) = cache {
        let key = FuentesCache::search_key(proxy.as_deref(), query);
        let binario_s = binario.to_string();
        let query_s = query.to_string();
        let proxy_s = proxy.clone();
        let settings_c = settings.clone();
        let limiter_c = limiter.clone();
        let error_state_c = error_state.cloned();

        let res = c
            .searches
            .wrap(
                &key,
                || async move {
                    buscar_cancion_uncached(
                        &binario_s,
                        &query_s,
                        proxy_s.as_deref(),
                        &settings_c,
                        &limiter_c,
                        prioridad,
                        error_state_c.as_ref(),
                    )
                    .await
                },
                Some(CACHE_BUSQUEDA_TTL_MS),
            )
            .await?;

        if let Some(ref song) = res {
            if let Some(ref s_url) = song.stream_url {
                let dummy = serde_json::json!({
                    "id": song.id,
                    "url": s_url,
                    "format_id": "bestaudio",
                });
                c.remember_info(proxy.as_deref(), &song.url, &dummy);
            }
        }
        Ok(res)
    } else {
        buscar_cancion_uncached(
            binario,
            query,
            proxy.as_deref(),
            settings,
            limiter,
            prioridad,
            error_state,
        )
        .await
    }
}

pub async fn buscar(
    binario: &str,
    query: &str,
    limit: usize,
    guild_id: Option<&str>,
    settings: &YtDlpSettings,
    proxies: &[String],
    limiter: &Limitador,
) -> Result<Vec<Cancion>, FuentesError> {
    let proxy = proxy_for(guild_id, proxies);
    let target = format!("ytsearch{limit}:{query}");
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

    let entries = val["entries"]
        .as_array()
        .ok_or(FuentesError::ResultadoVacio)?;
    let mut songs = Vec::new();
    for entry in entries {
        let id = entry["id"].as_str().unwrap_or("");
        if is_video_id(id) {
            songs.push(youtube_info(entry));
        }
    }
    Ok(songs)
}
