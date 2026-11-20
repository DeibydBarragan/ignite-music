//! Rutas para mover, ordenar y saltar canciones en la cola (Spec S-27).

use crate::api::contexto::EstadoApiContexto;
use crate::api::validacion::{es_snowflake, es_track_id_valido};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

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

#[derive(Debug, Deserialize)]
pub struct MoveBody {
    pub from: usize,
    pub to: usize,
}

pub async fn move_queue(
    State(st): State<EstadoApiContexto>,
    Path(gid): Path<String>,
    Json(b): Json<MoveBody>,
) -> Response {
    if let Some(e) = val_sf(&gid) {
        return e;
    }
    if b.from < 1 || b.to < 1 {
        return err(
            400,
            "INVALID_PARAMETER",
            "from and to must be integers of 1 or greater",
        );
    }
    if !st.gestor_colas.tiene_cola(&gid).await {
        return err(404, "NO_QUEUE", "No queue found");
    }
    let cola = st.gestor_colas.obtener_cola(&gid).await;
    let mut g = cola.lock().await;
    let last = g.canciones.len().saturating_sub(1);
    if b.from > last || b.to > last {
        return err(400, "INVALID_MOVE", "¡Posición inválida en la cola!");
    }
    let song = g.canciones.remove(b.from);
    let name = song.cancion.nombre.clone();
    g.canciones.insert(b.to, song);
    drop(g);
    st.sse.publicar_sincronizacion(&gid, &st).await;
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": format!("Movida \"{name}\" a la posición {}", b.to)
        })),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct OrderBody {
    #[serde(rename = "trackId")]
    pub track_id: String,
    pub to: usize,
}

pub async fn order_queue(
    State(st): State<EstadoApiContexto>,
    Path(gid): Path<String>,
    Json(b): Json<OrderBody>,
) -> Response {
    if let Some(e) = val_sf(&gid) {
        return e;
    }
    if !es_track_id_valido(&b.track_id) || b.to < 1 {
        return err(
            400,
            "INVALID_PARAMETER",
            "trackId must be valid and to must be 1 or greater",
        );
    }
    if !st.gestor_colas.tiene_cola(&gid).await {
        return err(404, "NO_QUEUE", "No queue found");
    }
    let cola = st.gestor_colas.obtener_cola(&gid).await;
    let mut g = cola.lock().await;
    let from = g
        .canciones
        .iter()
        .enumerate()
        .position(|(i, s)| i > 0 && s.track_id == b.track_id);
    let from = match from {
        Some(f) => f,
        None => {
            return err(
                404,
                "TRACK_NOT_FOUND",
                "No upcoming track with that trackId in this queue",
            );
        }
    };
    let last = g.canciones.len().saturating_sub(1);
    if b.to > last {
        return err(400, "INVALID_MOVE", "¡Posición inválida en la cola!");
    }
    let song = g.canciones.remove(from);
    let name = song.cancion.nombre.clone();
    g.canciones.insert(b.to, song);
    drop(g);
    st.sse.publicar_sincronizacion(&gid, &st).await;
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": format!("Movida \"{name}\" a la posición {}", b.to)
        })),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct JumpBody {
    pub position: usize,
}

pub async fn jump_queue(
    State(st): State<EstadoApiContexto>,
    Path(gid): Path<String>,
    Json(b): Json<JumpBody>,
) -> Response {
    if let Some(e) = val_sf(&gid) {
        return e;
    }
    if b.position < 1 {
        return err(
            400,
            "INVALID_PARAMETER",
            "position must be an integer of 1 or greater",
        );
    }
    if !st.gestor_colas.tiene_cola(&gid).await {
        return err(404, "NO_QUEUE", "No queue found");
    }
    let cola = st.gestor_colas.obtener_cola(&gid).await;
    let mut g = cola.lock().await;
    if b.position >= g.canciones.len() {
        return err(400, "INVALID_JUMP", "¡Posición inválida en la cola!");
    }
    let rem: Vec<_> = g.canciones.drain(0..b.position).collect();
    g.canciones_previas.extend(rem);
    g.pausada = false;
    let name = g
        .canciones
        .first()
        .map(|s| s.cancion.nombre.clone())
        .unwrap_or_default();
    drop(g);
    st.sse.publicar_sincronizacion(&gid, &st).await;
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": format!("Saltado a \"{name}\"")
        })),
    )
        .into_response()
}
