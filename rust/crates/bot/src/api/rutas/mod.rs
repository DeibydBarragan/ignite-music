//! Enrutador y registro de rutas v1 de la API REST (Spec S-27).

pub mod cola;
pub mod cola_orden;
pub mod descubrimiento;
mod descubrimiento_catalogo;
mod descubrimiento_helpers;
pub mod dev;
pub mod reproductor;
pub mod reproductor_control;
pub mod reproductor_play;
pub mod voz;

use super::contexto::EstadoApiContexto;
use axum::routing::{delete, get, patch, post, put};
use axum::Router;

fn crear_subrouter(estado: EstadoApiContexto) -> Router {
    Router::new()
        .route(
            "/guilds/{guildId}/player",
            get(reproductor::get_player_state),
        )
        .route("/guilds/{guildId}/play", post(reproductor::post_play))
        .route("/guilds/{guildId}/control", post(reproductor::post_control))
        .route("/guilds/{guildId}/events", get(reproductor::get_events))
        .route("/guilds/{guildId}/queue", delete(cola::clear_queue))
        .route("/guilds/{guildId}/queue/undo", post(cola::undo_queue))
        .route(
            "/guilds/{guildId}/queue/{index}",
            delete(cola::remove_index),
        )
        .route(
            "/guilds/{guildId}/queue/items/{trackId}",
            delete(cola::remove_track),
        )
        .route("/guilds/{guildId}/queue/move", post(cola::move_queue))
        .route("/guilds/{guildId}/queue/order", patch(cola::order_queue))
        .route("/guilds/{guildId}/queue/jump", post(cola::jump_queue))
        .route(
            "/guilds/{guildId}/top-songs",
            get(descubrimiento::get_top_songs),
        )
        .route(
            "/guilds/{guildId}/voice-status",
            get(descubrimiento::get_voice_status),
        )
        .route(
            "/guilds/{guildId}/channels",
            get(descubrimiento::get_channels),
        )
        .route(
            "/guilds/{guildId}/recommendations",
            get(descubrimiento::get_recommendations),
        )
        .route(
            "/guilds/{guildId}/playlist/resolve",
            get(descubrimiento::get_resolve_playlist),
        )
        .route("/guilds/{guildId}/voice", get(voz::get_voice_settings))
        .route("/guilds/{guildId}/voice", put(voz::put_voice_settings))
        .route(
            "/guilds/{guildId}/voice/reset",
            post(voz::reset_voice_settings),
        )
        .route("/guilds/{guildId}/voice/logs", get(voz::get_voice_logs))
        .route(
            "/guilds/{guildId}/voice/logs",
            delete(voz::clear_voice_logs),
        )
        .route("/guilds/{guildId}/voice/join", post(voz::join_voice))
        .route("/guilds/{guildId}/voice/leave", post(voz::leave_voice))
        .route("/guilds", get(descubrimiento::get_guilds))
        .route("/search", get(descubrimiento::get_search))
        .route("/search/suggest", get(descubrimiento::get_suggest))
        .route(
            "/search/playlists",
            get(descubrimiento::get_search_playlists),
        )
        .route("/user/dev-mode", get(dev::get_dev_mode))
        .route("/user/dev-mode", post(dev::post_dev_mode))
        .with_state(estado)
}

/// Construye el router con las 30 rutas montadas bajo `/api/v1` y el alias `/api`.
pub fn crear_rutas_v1(estado: EstadoApiContexto) -> Router {
    let sub = crear_subrouter(estado);
    Router::new().nest("/api/v1", sub.clone()).nest("/api", sub)
}
