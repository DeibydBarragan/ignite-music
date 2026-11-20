//! Rutas para el Modo Desarrollador (Spec S-27).

use crate::api::contexto::EstadoApiContexto;
use crate::api::seguridad::UsuarioDiscord;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct DevModeBody {
    pub enabled: Option<bool>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
}

fn resolver_usuario(
    ext: Option<&UsuarioDiscord>,
    headers: &HeaderMap,
    manual: Option<&str>,
) -> Option<String> {
    if let Some(UsuarioDiscord(id)) = ext {
        return Some(id.clone());
    }
    if let Some(h) = headers
        .get("x-user-discord-id")
        .and_then(|v| v.to_str().ok())
    {
        let t = h.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    manual
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub async fn get_dev_mode(
    State(estado): State<EstadoApiContexto>,
    usuario_ext: Option<Extension<UsuarioDiscord>>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let manual_id = query.get("userId").map(String::as_str);
    let uid = resolver_usuario(usuario_ext.as_deref(), &headers, manual_id);
    let dev_mode = match &uid {
        Some(id) => estado.dev_mode.es_dev_mode(id),
        None => false,
    };
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "devMode": dev_mode,
            "userId": uid
        })),
    )
        .into_response()
}

pub async fn post_dev_mode(
    State(estado): State<EstadoApiContexto>,
    usuario_ext: Option<Extension<UsuarioDiscord>>,
    headers: HeaderMap,
    Json(body): Json<DevModeBody>,
) -> Response {
    let uid = resolver_usuario(usuario_ext.as_deref(), &headers, body.user_id.as_deref());
    let uid = match uid {
        Some(id) => id,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "error": "MISSING_USER_ID",
                    "detail": "X-User-Discord-Id header or userId is required"
                })),
            )
                .into_response();
        }
    };

    let habilitado = body.enabled.unwrap_or(false);
    let updated = estado.dev_mode.establecer_dev_mode(&uid, habilitado);

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "devMode": updated,
            "userId": uid
        })),
    )
        .into_response()
}
