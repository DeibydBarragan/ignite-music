//! Manejo de acciones de control sobre el reproductor (Spec S-27).

use crate::api::contexto::EstadoApiContexto;
use crate::api::validacion::es_snowflake;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
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

pub const CONTROL_ACTIONS: &[&str] = &[
    "pause", "resume", "toggle", "skip", "previous", "stop", "shuffle", "autoplay", "filter",
    "volume", "seek", "loop", "deafen", "undeafen",
];

#[derive(Debug, Deserialize)]
pub struct ControlBody {
    pub action: Option<Value>,
    #[serde(default, deserialize_with = "valor_presente")]
    pub value: Option<Value>,
}

fn valor_presente<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

pub async fn post_control(
    State(st): State<EstadoApiContexto>,
    Path(gid): Path<String>,
    Json(b): Json<ControlBody>,
) -> Response {
    if let Some(e) = val_sf(&gid) {
        return e;
    }
    let Some(act) = b.action.as_ref().and_then(Value::as_str).map(str::trim) else {
        return err(400, "INVALID_PARAMETER", "action must be a string");
    };
    if !(1..=32).contains(&act.encode_utf16().count()) {
        return err(
            400,
            "INVALID_PARAMETER",
            "action must contain 1 to 32 characters",
        );
    }
    if b.value.as_ref().is_some_and(|value| match value {
        Value::Object(_) | Value::Array(_) => true,
        Value::String(s) => s.encode_utf16().count() > 200,
        _ => false,
    }) {
        return err(400, "INVALID_PARAMETER", "invalid control value");
    }
    if !CONTROL_ACTIONS.contains(&act) {
        return err(400, "INVALID_ACTION", &format!("Invalid action: {act}"));
    }
    if !st.gestor_colas.tiene_cola(&gid).await {
        return err(404, "NO_QUEUE", "No active playback queue in this guild");
    }
    let cola = st.gestor_colas.obtener_cola(&gid).await;
    let g = cola.lock().await;
    if g.canciones.is_empty() && act != "stop" {
        return err(404, "NO_QUEUE", "No active playback queue in this guild");
    }
    drop(g);

    // Some(null) is distinct from an omitted value for Node's numeric controls.
    match st.proveedor.controlar(&gid, act, b.value.as_ref()).await {
        Ok((ok, msg)) => {
            st.sse.publicar_sincronizacion(&gid, &st).await;
            (
                StatusCode::OK,
                Json(json!({ "success": ok, "message": msg })),
            )
                .into_response()
        }
        Err((status, code, detail)) => err(status, code, &detail),
    }
}
