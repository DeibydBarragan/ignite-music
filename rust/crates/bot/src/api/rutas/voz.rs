//! Rutas de voz y registros de interacción (Spec S-27).

use crate::api::contexto::EstadoApiContexto;
use crate::api::seguridad::UsuarioDiscord;
use crate::api::validacion::es_snowflake;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

#[path = "voz_validacion.rs"]
mod validacion;
use validacion::validar_cuerpo_ajustes;

fn validar_snowflake_ruta(guild_id: &str) -> Option<Response> {
    if !es_snowflake(guild_id) {
        Some(crate::api::respuesta_problema(
            400,
            "INVALID_SNOWFLAKE",
            "Guild ID must be a valid Discord snowflake.",
        ))
    } else {
        None
    }
}

pub async fn get_voice_settings(
    State(estado): State<EstadoApiContexto>,
    Path(guild_id): Path<String>,
) -> Response {
    if let Some(e) = validar_snowflake_ruta(&guild_id) {
        return e;
    }
    let data = estado.ajustes_voz.obtener_asincrono(&guild_id).await;
    (StatusCode::OK, Json(data)).into_response()
}

pub async fn put_voice_settings(
    State(estado): State<EstadoApiContexto>,
    Path(guild_id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    if let Some(e) = validar_snowflake_ruta(&guild_id) {
        return e;
    }
    if let Some(e) = validar_cuerpo_ajustes(&body) {
        return e;
    }

    match estado
        .ajustes_voz
        .actualizar_asincrono(&guild_id, body)
        .await
    {
        Ok(updated) => (StatusCode::OK, Json(updated)).into_response(),
        Err((status, code, msg)) => crate::api::respuesta_problema(status, code, &msg),
    }
}

pub async fn reset_voice_settings(
    State(estado): State<EstadoApiContexto>,
    Path(guild_id): Path<String>,
) -> Response {
    if let Some(e) = validar_snowflake_ruta(&guild_id) {
        return e;
    }
    let def = estado.ajustes_voz.reiniciar_asincrono(&guild_id).await;
    (StatusCode::OK, Json(def)).into_response()
}

pub async fn get_voice_logs(
    State(estado): State<EstadoApiContexto>,
    Path(guild_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if let Some(e) = validar_snowflake_ruta(&guild_id) {
        return e;
    }
    let limit = query
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(50);
    let logs = estado.logs_voz.obtener_logs(&guild_id, limit);
    (StatusCode::OK, Json(json!({ "logs": logs }))).into_response()
}

pub async fn clear_voice_logs(
    State(estado): State<EstadoApiContexto>,
    Path(guild_id): Path<String>,
) -> Response {
    if let Some(e) = validar_snowflake_ruta(&guild_id) {
        return e;
    }
    estado.logs_voz.limpiar(&guild_id);
    (StatusCode::OK, Json(json!({ "ok": true }))).into_response()
}

#[derive(Debug, Deserialize)]
pub struct JoinVoiceBody {
    #[serde(rename = "channelId")]
    pub channel_id: Option<String>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
}

pub async fn join_voice(
    State(estado): State<EstadoApiContexto>,
    Path(guild_id): Path<String>,
    usuario_ext: Option<Extension<UsuarioDiscord>>,
    body: Option<Json<JoinVoiceBody>>,
) -> Response {
    if let Some(e) = validar_snowflake_ruta(&guild_id) {
        return e;
    }
    let body_data = body.map(|Json(b)| b);
    let channel_id = body_data.as_ref().and_then(|b| b.channel_id.as_deref());
    let user_id = usuario_ext
        .as_deref()
        .map(|u| u.0.as_str())
        .or_else(|| body_data.as_ref().and_then(|b| b.user_id.as_deref()));

    match estado
        .proveedor
        .unirse_voz(&guild_id, channel_id, user_id)
        .await
    {
        Ok(canal) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "channel": { "id": canal.id, "name": canal.name },
                "message": format!("¡Conectado al canal {}!", canal.name)
            })),
        )
            .into_response(),
        Err((status, code, msg)) => crate::api::respuesta_problema(status, code, &msg),
    }
}

pub async fn leave_voice(
    State(estado): State<EstadoApiContexto>,
    Path(guild_id): Path<String>,
) -> Response {
    if let Some(e) = validar_snowflake_ruta(&guild_id) {
        return e;
    }
    if let Err((status, code, message)) = estado.proveedor.verificar_servidor(&guild_id).await {
        return crate::api::respuesta_problema(status, code, &message);
    }
    if let Err(error) = estado.proveedor.salir_voz(&guild_id).await {
        tracing::warn!(%error, %guild_id, "No se pudo salir del canal de voz");
        return crate::api::respuesta_problema(
            500,
            "voice_leave_failed",
            "No se pudo salir del canal de voz",
        );
    }
    if estado.gestor_colas.tiene_cola(&guild_id).await {
        let cola = estado.gestor_colas.obtener_cola(&guild_id).await;
        let mut guard = cola.lock().await;
        player::acciones::parar(&mut guard);
    }
    estado.sse.publicar_sincronizacion(&guild_id, &estado).await;
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Desconectado del canal de voz"
        })),
    )
        .into_response()
}
