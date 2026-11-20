//! Ejecución de peticiones de reproducción para el ProveedorApiReal (Spec S-27).

use super::formas::*;
use super::proveedor_real::ProveedorApiReal;
use medios::modelos::ResultadoResolucion;
use player::cola::ElementoCola;

#[allow(clippy::too_many_arguments)]
pub async fn ejecutar_reproduccion_api(
    prov: &ProveedorApiReal,
    guild_id: &str,
    query: &str,
    voice_channel_id: Option<&str>,
    caller_id: Option<&str>,
    next: bool,
    skip: bool,
    dev_mode: bool,
) -> Result<ResultadoReproducirApi, (u16, &'static str, String)> {
    let (servidor, canal) = super::proveedor_play_voz::seleccionar_canal(
        prov,
        guild_id,
        voice_channel_id,
        caller_id,
        dev_mode,
    )
    .await?;
    let canal_info = if prov.prueba_sin_union {
        CanalVozInfo {
            id: canal.get().to_string(),
            name: String::new(),
        }
    } else {
        super::proveedor_real_operaciones::conectar_canal_voz(prov, servidor.get(), canal.get())
            .await?
    };

    let bin = medios::resolve_binary(&prov.ytdlp_settings);
    let limiter = prov.ytdlp_settings.limiter.clone();
    let t0 = std::time::Instant::now();

    let resolucion = if let Some(falsa) = &prov.prueba_resolucion {
        falsa()
    } else if medios::parse_spotify_url(query).is_some() {
        let client = prov.spotify_client.as_ref().ok_or((
            400,
            "SPOTIFY_NOT_CONFIGURED",
            "Spotify client not configured".into(),
        ))?;
        client.resolve(query).await.map_err(|e| {
            let msg = e.to_string();
            tracing::error!("[{}] {}: {}", guild_id, query, msg);
            (422, "RESOLVE_FAILED", msg)
        })?
    } else if medios::parse_youtube_url(query).is_some() {
        medios::resolver_youtube(
            &bin,
            query,
            Some(guild_id),
            &prov.ytdlp_settings,
            &prov.proxies,
            &limiter,
            prov.max_playlist_size,
        )
        .await
        .map_err(|e| {
            let msg = e.to_string();
            tracing::error!("[{}] {}: {}", guild_id, query, msg);
            (422, "RESOLVE_FAILED", msg)
        })?
    } else if query.starts_with("http://") || query.starts_with("https://") {
        medios::resolver_generica(
            &bin,
            query,
            Some(guild_id),
            &prov.ytdlp_settings,
            &prov.proxies,
            &limiter,
            prov.max_playlist_size,
        )
        .await
        .map_err(|e| {
            let msg = e.to_string();
            tracing::error!("[{}] {}: {}", guild_id, query, msg);
            (422, "RESOLVE_FAILED", msg)
        })?
    } else {
        let canciones = medios::buscar(
            &bin,
            query,
            1,
            Some(guild_id),
            &prov.ytdlp_settings,
            &prov.proxies,
            &limiter,
        )
        .await
        .map_err(|e| {
            let msg = e.to_string();
            tracing::error!("[{}] {}: {}", guild_id, query, msg);
            (422, "SEARCH_FAILED", msg)
        })?;

        let cancion = canciones.into_iter().next().ok_or_else(|| {
            tracing::error!("[{}] {}: No track found", guild_id, query);
            (404, "NOT_FOUND", "No track found".into())
        })?;
        ResultadoResolucion::Cancion(cancion)
    };
    let found_secs = t0.elapsed().as_secs_f64();

    let solicitante = caller_id.and_then(|uid| {
        super::proveedor_real_aux_voz::datos_solicitante(&prov.cache, guild_id, uid)
    });
    let crear_elemento = |cancion| {
        let mut elem = ElementoCola::nuevo(cancion, caller_id.map(ToString::to_string));
        if let Some(user) = &solicitante {
            elem.requester_name = Some(user.username.clone());
            elem.requester_avatar = user.avatar.clone();
        }
        elem
    };

    let (elementos, mut playlist_info) = match resolucion {
        ResultadoResolucion::Cancion(cancion) => (vec![crear_elemento(cancion)], None),
        ResultadoResolucion::Lista(lista) => {
            let count = lista.canciones.len();
            let name = Some(lista.nombre.clone());
            let pl_info = Some(InfoPlaylistPlay {
                name,
                count,
                truncated: lista.truncada,
            });
            let elems = lista.canciones.into_iter().map(crear_elemento).collect();
            (elems, pl_info)
        }
    };

    if elementos.is_empty() {
        return Err((400, "EMPTY_PLAYLIST", "No tracks found to play".into()));
    }

    let cola_arc = prov.gestor_colas.obtener_cola(guild_id).await;
    let mut cola = cola_arc.lock().await;
    let anterior = (skip || cola.canciones.is_empty()).then(|| cola.clone());

    let capacidad = cola.capacidad_restante(prov.max_queue_size);
    let pos = if next { 1 } else { 0 };
    let res_encolar = player::encolar(
        &mut cola,
        elementos.clone(),
        pos,
        skip,
        prov.max_queue_size,
        true,
    )
    .map_err(|e| match e {
        player::ErrorReproducir::ColaLlena => (400, "QUEUE_FULL", "Queue is full".into()),
        other => (400, "ENQUEUE_ERROR", other.to_string()),
    })?;
    cola.canal_voz = Some(canal_info.id);
    if let Some(info) = &mut playlist_info {
        info.truncated |= res_encolar.truncado;
        info.count = elementos.len().min(capacidad);
    }

    let song_front = cola
        .canciones
        .get(res_encolar.posicion)
        .map(|e| serializar_elemento_cola(e, false));
    if res_encolar.empezada {
        cola.posicion_segundos = 0;
        if let Some(drv) = &prov.driver {
            let proxy = medios::proxy_for(Some(guild_id), &prov.proxies);
            let args = medios::base_args(&prov.ytdlp_settings, proxy.as_deref(), None);
            if let Err(e) = player::reproduccion::iniciar_pista_con_settings(
                drv,
                &mut cola,
                &bin,
                args,
                &prov.ytdlp_settings,
            )
            .await
            {
                if let Some(anterior) = anterior {
                    *cola = anterior;
                }
                tracing::error!("[{}] Error arrancando reproduccion: {}", guild_id, e);
                return Err((500, "PLAYBACK_FAILED", e.to_string()));
            }
        }
    }
    drop(cola);

    if res_encolar.empezada {
        if let Some(first) = elementos.first() {
            let audio_secs = t0.elapsed().as_secs_f64();
            tracing::info!(
                "Playing \"{}\" from {}: found in {:.1} s, audio after {:.1} s",
                first.cancion.nombre,
                first.cancion.fuente,
                found_secs,
                audio_secs
            );
        }
    }

    Ok(ResultadoReproducirApi {
        success: true,
        started: res_encolar.empezada,
        position: res_encolar.posicion,
        song: song_front,
        playlist: playlist_info,
    })
}

#[cfg(test)]
#[path = "post_play_historial_pruebas.rs"]
mod pruebas;
