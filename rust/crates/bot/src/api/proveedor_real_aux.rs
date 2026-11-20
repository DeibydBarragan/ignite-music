//! Funciones auxiliares de medios para el ProveedorApiReal (Spec S-27).

use super::formas::*;
use super::proveedor_real::ProveedorApiReal;
use medios::ytdlp_args::YtDlpSettings;
use player::cola::ElementoCola;

/// Extensión del icono de servidor: `gif` si el hash empieza por `a_` (animado), si no `webp`.
pub fn extension_icono(hash: &str) -> &'static str {
    if hash.starts_with("a_") {
        "gif"
    } else {
        "webp"
    }
}

/// Cola las `relacionadas` para la canción que acaba de terminar (si la
/// previa existe y es de YouTube). Excluye candidatas ya tocadas en esta
/// sesión de cola para evitar bucles; si todas están repetidas, acepta
/// cualquiera distinta a la actual.
pub async fn encolar_relacionado(
    cola: &mut player::Cola,
    guild_id: &str,
    ytdlp_settings: &YtDlpSettings,
    proxies: &[String],
) -> Result<bool, String> {
    let bin = medios::resolve_binary(ytdlp_settings);
    let limiter = ytdlp_settings.limiter.clone();

    let Some(previa) = cola.canciones_previas.last().cloned() else {
        return Ok(false);
    };
    let Some(medios::YouTubeUrlType::Video { id }) = medios::parse_youtube_url(&previa.cancion.url)
    else {
        return Ok(false);
    };

    let relacion =
        medios::relacionados(&bin, &id, Some(guild_id), ytdlp_settings, proxies, &limiter)
            .await
            .map_err(|e| format!("relacionados falló: {e}"))?;

    let current = previa.cancion.id;
    let previas: std::collections::HashSet<&str> = cola
        .canciones_previas
        .iter()
        .map(|c| c.cancion.id.as_str())
        .collect();

    let pick = relacion
        .iter()
        .find(|c| c.id != current && !previas.contains(c.id.as_str()))
        .or_else(|| relacion.iter().find(|c| c.id != current))
        .cloned();

    if let Some(cancion) = pick {
        cola.canciones.push(ElementoCola::nuevo(cancion, None));
        return Ok(true);
    }
    Ok(false)
}

pub async fn buscar_en_medios(
    q: &str,
    l: usize,
    guild_id: Option<&str>,
    settings: &YtDlpSettings,
    proxies: &[String],
) -> Result<Vec<PistaFrontend>, String> {
    let bin = medios::resolve_binary(settings);
    let limiter = settings.limiter.clone();
    let canciones = medios::buscar(&bin, q, l, guild_id, settings, proxies, &limiter)
        .await
        .map_err(|e| format!("Error en búsqueda: {e}"))?;
    Ok(canciones
        .into_iter()
        .map(|c| {
            let elem = player::ElementoCola::nuevo(c, None);
            serializar_elemento_cola(&elem, false)
        })
        .collect())
}

pub async fn sugerir_consultas(client: &reqwest::Client, q: &str) -> Result<Vec<String>, String> {
    if q.trim().is_empty() {
        return Ok(Vec::new());
    }
    let encoded: String = url::form_urlencoded::byte_serialize(q.trim().as_bytes()).collect();
    let url = format!(
        "https://suggestqueries.google.com/complete/search?client=firefox&ds=yt&q={encoded}"
    );
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let json_val: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    if let Some(arr) = json_val.get(1).and_then(serde_json::Value::as_array) {
        let sugs = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        Ok(sugs)
    } else {
        Ok(Vec::new())
    }
}

pub async fn buscar_listas_en_medios(
    q: &str,
    l: usize,
    guild_id: Option<&str>,
    settings: &YtDlpSettings,
    proxies: &[String],
) -> Result<Vec<PlaylistFrontend>, String> {
    let bin = medios::resolve_binary(settings);
    let limiter = settings.limiter.clone();
    let canciones = medios::buscar_listas(&bin, q, l, guild_id, settings, proxies, &limiter)
        .await
        .map_err(|e| format!("Error en búsqueda de playlists: {e}"))?;
    Ok(canciones
        .into_iter()
        .map(|c| {
            let thumb = c.miniatura.unwrap_or_else(|| FALLBACK_COVER.to_string());
            PlaylistFrontend {
                id: c.id,
                title: c.nombre.clone(),
                name: c.nombre.clone(),
                uploader: c.autor.clone(),
                author: c.autor,
                thumbnail: thumb.clone(),
                cover: thumb,
                track_count: c.cantidad_pistas,
                url: c.url,
                source: "youtube".into(),
            }
        })
        .collect())
}

pub(super) async fn resolver_lista_en_medios(
    proveedor: &ProveedorApiReal,
    guild_id: &str,
    url: &str,
) -> Result<ResultadoPlaylistFrontend, String> {
    let settings = &proveedor.ytdlp_settings;
    let bin = medios::resolve_binary(settings);
    let res = medios::resolve_playlist(
        url,
        None,
        Some(proveedor.max_playlist_size),
        proveedor.spotify_client.as_deref(),
        &bin,
        Some(guild_id),
        settings,
        &proveedor.proxies,
        &settings.limiter,
    )
    .await
    .map_err(|e| format!("No se pudo resolver la playlist: {e}"))?;
    Ok(ResultadoPlaylistFrontend {
        success: true,
        kind: res.kind,
        name: res.name,
        source: res.source,
        count: res.tracks.len(),
        truncated: res.truncated,
        tracks: res.tracks,
    })
}

pub use super::proveedor_real_aux_voz::{obtener_canales_cache, obtener_estado_voz_cache};
