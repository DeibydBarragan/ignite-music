//! Rutas para operaciones sobre la cola de reproducción (Spec S-27).

pub use super::cola_orden::{jump_queue, move_queue, order_queue, JumpBody, MoveBody, OrderBody};

use crate::api::contexto::EstadoApiContexto;
use crate::api::formas::serializar_elemento_cola;
use crate::api::validacion::{es_snowflake, es_track_id_valido};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use player::cola_operaciones::UNDO_TTL_MS;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

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

fn ahora_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub async fn clear_queue(State(st): State<EstadoApiContexto>, Path(gid): Path<String>) -> Response {
    if let Some(e) = val_sf(&gid) {
        return e;
    }
    if !st.gestor_colas.tiene_cola(&gid).await {
        return err(404, "NO_QUEUE", "No queue found");
    }
    let cola = st.gestor_colas.obtener_cola(&gid).await;
    let mut g = cola.lock().await;
    if g.canciones.is_empty() {
        return err(404, "NO_QUEUE", "No queue found");
    }
    let count = g.canciones.len().saturating_sub(1);
    if g.canciones.len() > 1 {
        let rem: Vec<_> = g.canciones.drain(1..).collect();
        st.gestor_colas
            .guardar_deshacer(&gid, rem, ahora_ms() + UNDO_TTL_MS)
            .await;
    }
    drop(g);
    st.sse.publicar_sincronizacion(&gid, &st).await;
    (
        StatusCode::OK,
        Json(json!({ "success": true, "cleared": count })),
    )
        .into_response()
}

pub async fn undo_queue(State(st): State<EstadoApiContexto>, Path(gid): Path<String>) -> Response {
    if let Some(e) = val_sf(&gid) {
        return e;
    }
    let entry = match st.gestor_colas.obtener_deshacer(&gid).await {
        Some(e) if !e.canciones.is_empty() => e,
        _ => {
            return err(
                400,
                "UNDO_EMPTY",
                "No hay canciones recientes para restaurar.",
            )
        }
    };
    if ahora_ms() > entry.expira_en {
        return err(400, "UNDO_EXPIRED", "El tiempo para deshacer ha expirado.");
    }
    if !st.gestor_colas.tiene_cola(&gid).await {
        return err(
            404,
            "NO_ACTIVE_PLAYER",
            "El reproductor no está activo en este servidor.",
        );
    }
    let cola = st.gestor_colas.obtener_cola(&gid).await;
    let mut g = cola.lock().await;
    let n = entry.canciones.len();
    g.canciones.extend(entry.canciones);
    drop(g);
    st.sse.publicar_sincronizacion(&gid, &st).await;
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "restored": n,
            "message": format!("{n} canciones restauradas a la cola.")
        })),
    )
        .into_response()
}

pub async fn remove_index(
    State(st): State<EstadoApiContexto>,
    Path((gid, idx_str)): Path<(String, String)>,
) -> Response {
    if let Some(e) = val_sf(&gid) {
        return e;
    }
    let idx = match idx_str.parse::<usize>() {
        Ok(i) if i >= 1 => i,
        _ => {
            return err(
                400,
                "INVALID_PARAMETER",
                "index must be an integer of 1 or greater",
            );
        }
    };
    if !st.gestor_colas.tiene_cola(&gid).await {
        return err(404, "NO_QUEUE", "No queue found");
    }
    let cola = st.gestor_colas.obtener_cola(&gid).await;
    let mut g = cola.lock().await;
    if idx >= g.canciones.len() {
        return err(400, "INVALID_INDEX", "Invalid song queue index");
    }
    let rem = g.canciones.remove(idx);
    let s = serializar_elemento_cola(&rem, false);
    drop(g);
    st.sse.publicar_sincronizacion(&gid, &st).await;
    (
        StatusCode::OK,
        Json(json!({ "success": true, "removed": s })),
    )
        .into_response()
}

pub async fn remove_track(
    State(st): State<EstadoApiContexto>,
    Path((gid, tid)): Path<(String, String)>,
) -> Response {
    if let Some(e) = val_sf(&gid) {
        return e;
    }
    if !es_track_id_valido(&tid) {
        return err(400, "INVALID_PARAMETER", "must be a valid track id");
    }
    if !st.gestor_colas.tiene_cola(&gid).await {
        return err(404, "NO_QUEUE", "No queue found");
    }
    let cola = st.gestor_colas.obtener_cola(&gid).await;
    let mut g = cola.lock().await;
    let idx = g
        .canciones
        .iter()
        .enumerate()
        .position(|(i, s)| i > 0 && s.track_id == tid);
    let idx = match idx {
        Some(i) => i,
        None => {
            return err(
                404,
                "TRACK_NOT_FOUND",
                "No upcoming track with that trackId in this queue",
            );
        }
    };
    let rem = g.canciones.remove(idx);
    let s = serializar_elemento_cola(&rem, false);
    drop(g);
    st.sse.publicar_sincronizacion(&gid, &st).await;
    (
        StatusCode::OK,
        Json(json!({ "success": true, "removed": s })),
    )
        .into_response()
}
