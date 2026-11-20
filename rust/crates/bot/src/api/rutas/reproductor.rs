//! Rutas del reproductor: estado, play y eventos SSE (Spec S-27, S-28).

pub use super::reproductor_control::{post_control, ControlBody, CONTROL_ACTIONS};
pub use super::reproductor_play::{post_play, PlayBody};

use crate::api::contexto::{EstadoApiContexto, ProveedorApi};
use crate::api::formas::*;
use crate::api::validacion::es_snowflake;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use player::cola::ModoRepeticion;
use serde_json::{json, Value};

fn err(status: u16, code: &'static str, detail: &str) -> Response {
    crate::api::respuesta_problema(status, code, detail)
}

fn val_sf(id: &str) -> Option<Response> {
    if !es_snowflake(id) {
        Some(err(
            400,
            "INVALID_SNOWFLAKE",
            "Guild ID must be a valid Discord snowflake.",
        ))
    } else {
        None
    }
}

pub fn estado_vacio(vol: u8, auto: bool) -> Value {
    json!({
        "hasQueue": false, "isPlaying": false, "isPaused": false, "currentTime": 0, "duration": 0,
        "volume": vol, "repeatMode": "off", "autoplay": auto, "filter": null, "voiceChannel": null,
        "currentSong": null, "queue": [], "disconnectReason": null
    })
}

pub async fn calcular_estado_reproductor(st: &EstadoApiContexto, gid: &str) -> Value {
    calcular_estado_reproductor_con(&st.gestor_colas, st.proveedor.as_ref(), gid).await
}

pub(crate) async fn calcular_estado_reproductor_con(
    colas: &player::GestorColas,
    proveedor: &dyn ProveedorApi,
    gid: &str,
) -> Value {
    if !colas.tiene_cola(gid).await {
        return estado_vacio(player::DEFAULT_VOLUME, false);
    }
    let live = proveedor.obtener_posicion_en_vivo(gid).await;
    let cola = colas.obtener_cola(gid).await;
    let g = cola.lock().await;
    if g.canciones.is_empty() {
        return estado_vacio(g.volumen, g.autoplay);
    }
    let cur = g
        .canciones
        .first()
        .map(|s| serializar_elemento_cola(s, g.autoplay));
    let upcoming: Vec<_> = g
        .canciones
        .iter()
        .skip(1)
        .map(|s| serializar_elemento_cola(s, g.autoplay))
        .collect();
    let rep = match g.modo_repeticion {
        ModoRepeticion::Cancion => "track",
        ModoRepeticion::Cola => "queue",
        _ => "off",
    };
    let (current_time, is_playing) = match live {
        Some((pos, playing)) => (pos, playing),
        None => (g.posicion_segundos, !g.pausada),
    };
    let voice_channel = g
        .canal_voz
        .as_deref()
        .and_then(|channel_id| proveedor.obtener_info_canal_voz(gid, channel_id));
    json!({
        "hasQueue": true,
        "isPlaying": is_playing,
        "isPaused": g.pausada,
        "currentTime": current_time,
        "duration": cur.as_ref().map(|c| c.duration).unwrap_or(0),
        "volume": g.volumen,
        "repeatMode": rep,
        "autoplay": g.autoplay,
        "filter": g.filtros.first().cloned(),
        "voiceChannel": voice_channel,
        "currentSong": cur,
        "queue": upcoming,
        "disconnectReason": null
    })
}

pub async fn get_player_state(
    State(st): State<EstadoApiContexto>,
    Path(gid): Path<String>,
) -> Response {
    if let Some(e) = val_sf(&gid) {
        return e;
    }
    if let Err((status, code, msg)) = st.proveedor.verificar_servidor(&gid).await {
        let sc = match status {
            404 => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        return err(sc.as_u16(), code, &msg);
    }
    let estado = calcular_estado_reproductor(&st, &gid).await;
    (StatusCode::OK, Json(estado)).into_response()
}

pub async fn get_events(State(st): State<EstadoApiContexto>, Path(gid): Path<String>) -> Response {
    if let Some(e) = val_sf(&gid) {
        return e;
    }
    if let Err((status, code, msg)) = st.proveedor.verificar_servidor(&gid).await {
        let sc = match status {
            404 => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        return err(sc.as_u16(), code, &msg);
    }
    let estado_inicial = calcular_estado_reproductor(&st, &gid).await;
    st.sse.suscribir_con_inicial(&gid, estado_inicial)
}
