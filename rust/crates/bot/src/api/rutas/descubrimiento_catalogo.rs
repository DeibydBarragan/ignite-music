//! Rutas con resultados de ranking, radio y resolución de playlists.

use super::descubrimiento::{err, val_sf};
use super::descubrimiento_helpers::limite_query;
use crate::api::contexto::EstadoApiContexto;
use crate::api::validacion::{inspeccionar_destino, razon_url_no_soportada};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use std::collections::HashMap;

pub async fn get_recommendations(
    State(st): State<EstadoApiContexto>,
    Path(gid): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if let Some(e) = val_sf(&gid) {
        return e;
    }
    if let Err((status, code, detail)) = st.proveedor.verificar_servidor(&gid).await {
        return err(status, code, &detail);
    }
    let force = ["force", "refresh"]
        .iter()
        .any(|key| query.get(*key).is_some_and(|v| v == "true"));
    let rec = st
        .proveedor
        .recomendaciones(&gid, force)
        .await
        .unwrap_or_default();
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "source": rec.source,
            "basedOn": rec.based_on,
            "songs": rec.songs,
        })),
    )
        .into_response()
}

pub async fn get_top_songs(
    State(st): State<EstadoApiContexto>,
    Path(gid): Path<String>,
    Query(q_map): Query<HashMap<String, String>>,
) -> Response {
    if let Some(e) = val_sf(&gid) {
        return e;
    }
    let timeframe = q_map.get("timeframe").map(String::as_str).unwrap_or("week");
    if timeframe != "week" && timeframe != "month" && timeframe != "all" {
        return err(
            400,
            "INVALID_PARAMETER",
            "timeframe must be week, month, or all",
        );
    }
    let Some(limit) = limite_query(&q_map, 100, 100) else {
        return err(
            400,
            "INVALID_PARAMETER",
            "limit is outside the allowed range",
        );
    };
    if let Err((status, code, detail)) = st.proveedor.verificar_servidor(&gid).await {
        return err(status, code, &detail);
    }
    let limit = limit.min(crate::datos::ranking::TOP_SONGS_LIMIT);
    let mut songs = st
        .proveedor
        .top_songs(&gid, timeframe, limit)
        .await
        .unwrap_or_default();
    songs.truncate(limit);
    let total = songs.len();
    (
        StatusCode::OK,
        Json(json!({
            "success": true, "guildId": gid, "timeframe": timeframe, "total": total, "songs": songs
        })),
    )
        .into_response()
}

pub async fn get_resolve_playlist(
    State(st): State<EstadoApiContexto>,
    Path(gid): Path<String>,
    Query(q_map): Query<HashMap<String, String>>,
) -> Response {
    if let Some(e) = val_sf(&gid) {
        return e;
    }
    let url = match q_map.get("url").map(|s| s.trim()).filter(|s| !s.is_empty()) {
        Some(u) if u.encode_utf16().count() <= 1000 => u,
        _ => return err(400, "INVALID_PARAMETER", "url parameter is required"),
    };
    if let Some((code, detail)) = inspeccionar_destino(url) {
        return err(400, code, &detail);
    }
    if let Err((status, code, detail)) = st.proveedor.verificar_servidor(&gid).await {
        return err(status, code, &detail);
    }
    if let Some(reason) = razon_url_no_soportada(url) {
        return err(422, "UNSUPPORTED_URL", reason);
    }
    match st.proveedor.resolver_playlist(&gid, url).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(e) => err(422, "RESOLVE_FAILED", &e),
    }
}
