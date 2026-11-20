//! Rutas de descubrimiento: búsqueda, sugerencias, recomendaciones y canales (Spec S-27).

pub use super::descubrimiento_catalogo::{
    get_recommendations, get_resolve_playlist, get_top_songs,
};
use super::descubrimiento_helpers::{
    consulta_busqueda_valida, limite_query, puede_controlar_voz, texto_query,
};
use crate::api::contexto::EstadoApiContexto;
use crate::api::validacion::{es_snowflake, es_snowflake_opcional};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use std::collections::HashMap;

pub(super) fn err(status: u16, code: &'static str, detail: &str) -> Response {
    crate::api::respuesta_problema(status, code, detail)
}

pub(super) fn val_sf(id: &str) -> Option<Response> {
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

pub async fn get_guilds(State(st): State<EstadoApiContexto>) -> Response {
    let guilds = st.proveedor.obtener_servidores().await;
    (StatusCode::OK, Json(json!({ "guilds": guilds }))).into_response()
}

pub async fn get_search(
    State(st): State<EstadoApiContexto>,
    Query(q_map): Query<HashMap<String, String>>,
) -> Response {
    let Some(q) = texto_query(&q_map, "q") else {
        return err(
            400,
            "INVALID_PARAMETER",
            "query must be at most 500 characters",
        );
    };
    let Some(limit) = limite_query(&q_map, 6, 15) else {
        return err(
            400,
            "INVALID_PARAMETER",
            "limit is outside the allowed range",
        );
    };
    if q_map
        .get("type")
        .is_some_and(|kind| kind != "video" && kind != "playlist")
    {
        return err(400, "INVALID_PARAMETER", "type must be video or playlist");
    }
    let is_pl = q_map.get("type").map(|s| s == "playlist").unwrap_or(false);
    if !consulta_busqueda_valida(q) {
        return (StatusCode::OK, Json(json!({ "query": q, "results": [] }))).into_response();
    }

    if is_pl {
        match st.proveedor.buscar_playlists(q, limit, None).await {
            Ok(res) => {
                (StatusCode::OK, Json(json!({ "query": q, "results": res }))).into_response()
            }
            Err(_) => (StatusCode::OK, Json(json!({ "query": q, "results": [] }))).into_response(),
        }
    } else {
        match st.proveedor.buscar(q, limit, None).await {
            Ok(res) => {
                (StatusCode::OK, Json(json!({ "query": q, "results": res }))).into_response()
            }
            Err(_) => (StatusCode::OK, Json(json!({ "query": q, "results": [] }))).into_response(),
        }
    }
}

pub async fn get_suggest(
    State(st): State<EstadoApiContexto>,
    Query(q_map): Query<HashMap<String, String>>,
) -> Response {
    let Some(q) = texto_query(&q_map, "q") else {
        return err(
            400,
            "INVALID_PARAMETER",
            "query must be at most 500 characters",
        );
    };
    if !consulta_busqueda_valida(q) {
        return (
            StatusCode::OK,
            Json(json!({ "query": q, "suggestions": [] })),
        )
            .into_response();
    }
    match st.proveedor.sugerir(q).await {
        Ok(sug) => (
            StatusCode::OK,
            Json(json!({ "query": q, "suggestions": sug })),
        )
            .into_response(),
        Err(_) => (
            StatusCode::OK,
            Json(json!({ "query": q, "suggestions": [] })),
        )
            .into_response(),
    }
}

pub async fn get_search_playlists(
    State(st): State<EstadoApiContexto>,
    Query(q_map): Query<HashMap<String, String>>,
) -> Response {
    let Some(q) = texto_query(&q_map, "q") else {
        return err(
            400,
            "INVALID_PARAMETER",
            "query must be at most 500 characters",
        );
    };
    let Some(limit) = limite_query(&q_map, 6, 15) else {
        return err(
            400,
            "INVALID_PARAMETER",
            "limit is outside the allowed range",
        );
    };
    if !consulta_busqueda_valida(q) {
        return (StatusCode::OK, Json(json!({ "query": q, "results": [] }))).into_response();
    }
    match st.proveedor.buscar_playlists(q, limit, None).await {
        Ok(res) => (StatusCode::OK, Json(json!({ "query": q, "results": res }))).into_response(),
        Err(_) => (StatusCode::OK, Json(json!({ "query": q, "results": [] }))).into_response(),
    }
}

pub async fn get_voice_status(
    State(st): State<EstadoApiContexto>,
    Path(gid): Path<String>,
    Query(q_map): Query<HashMap<String, String>>,
) -> Response {
    if let Some(e) = val_sf(&gid) {
        return e;
    }
    let uid = q_map.get("userId").map(String::as_str);
    if !es_snowflake_opcional(uid) {
        return err(400, "INVALID_PARAMETER", "userId must be a valid snowflake");
    }
    match st.proveedor.estado_voz(&gid, uid).await {
        Ok(mut status) => {
            let dev_user =
                uid.is_some_and(|id| st.dev_mode.es_dev_user(id, &st.config.dev_user_ids));
            status.can_control = puede_controlar_voz(&status, dev_user);
            (StatusCode::OK, Json(status)).into_response()
        }
        Err(e) if e == "guild_not_found" => err(404, "guild_not_found", "Guild not found"),
        Err(e) => err(500, "op_failed", &e),
    }
}

pub async fn get_channels(
    State(st): State<EstadoApiContexto>,
    Path(gid): Path<String>,
) -> Response {
    if let Some(e) = val_sf(&gid) {
        return e;
    }
    match st.proveedor.obtener_canales(&gid).await {
        Ok(channels) => (
            StatusCode::OK,
            Json(json!({ "guildId": gid, "total": channels.len(), "channels": channels })),
        )
            .into_response(),
        Err(e) => err(404, "guild_not_found", &e),
    }
}
