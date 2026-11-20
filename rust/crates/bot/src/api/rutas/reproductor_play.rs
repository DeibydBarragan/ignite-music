//! Manejo de peticiones de reproducción /play (Spec S-27).

use crate::api::contexto::EstadoApiContexto;
use crate::api::seguridad::UsuarioDiscord;
use crate::api::validacion::{
    es_snowflake, es_snowflake_opcional, inspeccionar_destino, razon_url_no_soportada,
};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;

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
pub struct PlayBody {
    pub query: Option<String>,
    #[serde(rename = "voiceChannelId")]
    pub voice_channel_id: Option<String>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    pub next: Option<bool>,
    pub skip: Option<bool>,
}

pub async fn post_play(
    State(st): State<EstadoApiContexto>,
    Path(gid): Path<String>,
    headers: HeaderMap,
    usuario_auth: Option<Extension<UsuarioDiscord>>,
    Json(b): Json<PlayBody>,
) -> Response {
    if let Some(e) = val_sf(&gid) {
        return e;
    }
    let q = match b.query.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => s,
        _ => return err(400, "INVALID_PARAMETER", "Field 'query' is required"),
    };
    if q.encode_utf16().count() > 1000 {
        return err(
            400,
            "INVALID_PARAMETER",
            "query: Too big: expected string to have <=1000 characters",
        );
    }
    if let Some((code, detail)) = inspeccionar_destino(q) {
        return err(400, code, &detail);
    }
    if let Some(reason) = razon_url_no_soportada(q) {
        return err(400, "UNSUPPORTED_URL", reason);
    }
    if let Some(ref cid) = b.voice_channel_id {
        if !cid.is_empty() && !es_snowflake_opcional(Some(cid)) {
            return err(
                400,
                "INVALID_PARAMETER",
                "Field 'voiceChannelId' must be a snowflake",
            );
        }
    }
    if let Some(ref uid) = b.user_id {
        if !uid.is_empty() && !es_snowflake_opcional(Some(uid)) {
            return err(
                400,
                "INVALID_PARAMETER",
                "Field 'userId' must be a snowflake",
            );
        }
    }

    let caller_id = usuario_auth
        .map(|Extension(u)| u.0)
        .or_else(|| b.user_id.clone());

    let header_dev_mode = headers
        .get("x-dev-mode")
        .and_then(|value| value.to_str().ok())
        == Some("true");
    let is_dev = header_dev_mode
        || caller_id
            .as_deref()
            .map(|uid| st.dev_mode.es_dev_user(uid, &st.config.dev_user_ids))
            .unwrap_or(false);

    match st
        .proveedor
        .reproducir(
            &gid,
            q,
            b.voice_channel_id.as_deref().filter(|id| !id.is_empty()),
            caller_id.as_deref(),
            b.next.unwrap_or(false),
            b.skip.unwrap_or(false),
            is_dev,
        )
        .await
    {
        Ok(resultado) => {
            st.sse.publicar_sincronizacion(&gid, &st).await;
            (StatusCode::OK, Json(resultado)).into_response()
        }
        Err((status, code, detail)) => err(status, code, &detail),
    }
}
