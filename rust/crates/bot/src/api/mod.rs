//! Servidor REST de la API y capas de seguridad (Spec S-26).

pub mod almacen_voz;
pub mod anuncio;
pub mod capas;
pub mod ciclo_vida;
pub mod contexto;
pub mod dev_mode;
mod eventos_reproduccion;
pub mod formas;
pub mod proveedor_control;
mod proveedor_play_voz;
pub mod proveedor_real;
pub mod proveedor_real_aux;
mod proveedor_real_aux_voz;
mod proveedor_real_operaciones;
mod proveedor_recomendaciones;
pub mod proveedor_reproduccion;
pub mod rate_limit;
pub mod rutas;
pub mod seguridad;
pub mod sse;
pub mod sse_stream;
pub mod tunel;
pub mod validacion;

pub use anuncio::{
    calcular_url_publica, Anunciador, ClienteSupabase, ClienteSupabaseHttp,
    DEFAULT_ANNOUNCE_INTERVAL,
};
pub use capas::{generar_request_id, RequestId, MAX_BODY_BYTES};
pub use ciclo_vida::ServiciosApiBot;
pub use contexto::{EstadoApiContexto, GestorDevMode, ProveedorApi};
pub use proveedor_real::ProveedorApiReal;
pub use rate_limit::EstadoRateLimit;
pub use seguridad::{
    es_metodo_mutacion, es_snowflake_valido, extract_bearer, safe_equal, UsuarioDiscord,
    MIN_SECRET_LENGTH,
};
pub use sse::{HubSse, DEFAULT_HEARTBEAT_MS};
pub use tunel::{extraer_url_tunel, resolver_binario, GestorTunel, DEFAULT_TUNNEL_TIMEOUT_MS};

use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use nucleo::ProblemDocument;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::oneshot;

/// Construye una respuesta HTTP con formato RFC 7807 problem+json.
pub fn respuesta_problema(status: u16, code: &str, detail: &str) -> Response {
    let doc = ProblemDocument::new(status, code, detail);
    respuesta_problema_doc(doc)
}

/// Serializa un ProblemDocument existente en una respuesta HTTP RFC 7807.
pub fn respuesta_problema_doc(doc: ProblemDocument) -> Response {
    let http_status = StatusCode::from_u16(doc.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = serde_json::to_string(&doc).unwrap_or_default();
    let mut res = Response::new(axum::body::Body::from(body));
    *res.status_mut() = http_status;
    res.headers_mut().insert(
        CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/problem+json; charset=utf-8"),
    );
    res
}

#[derive(Clone)]
struct EstadoApi {
    inicio: Instant,
}

async fn health_handler(State(estado): State<EstadoApi>) -> Response {
    let uptime = estado.inicio.elapsed().as_secs();
    let body = json!({
        "status": "ok",
        "uptime": uptime
    });
    let mut res = Response::new(axum::body::Body::from(
        serde_json::to_string(&body).unwrap_or_default(),
    ));
    *res.status_mut() = StatusCode::OK;
    res.headers_mut().insert(
        CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json; charset=utf-8"),
    );
    res
}

async fn fallback_404() -> Response {
    respuesta_problema(404, "NOT_FOUND", "Ruta no encontrada.")
}

/// Crea el router de la API aplicando todas las capas de seguridad y rutas v1 por defecto.
pub fn crear_router(config: Arc<nucleo::Config>, inicio: Instant) -> Router {
    let gestor = Arc::new(player::GestorColas::nuevo());
    let ctx = contexto::EstadoApiContexto::nuevo(config.clone(), gestor);
    crear_router_con_contexto(inicio, ctx)
}

/// Crea el router de la API con un contexto explícito.
pub fn crear_router_con_contexto(inicio: Instant, ctx: contexto::EstadoApiContexto) -> Router {
    let config = ctx.config.clone();
    let rutas_v1 = rutas::crear_rutas_v1(ctx);
    crear_router_con_rutas(config, inicio, rutas_v1)
}

/// Crea el router de la API permitiendo inyectar rutas protegidas adicionales (para tests o S-27).
pub fn crear_router_con_rutas(
    config: Arc<nucleo::Config>,
    inicio: Instant,
    rutas_personalizadas: Router,
) -> Router {
    let estado_rate_limit = EstadoRateLimit::default();
    let estado_api = EstadoApi { inicio };

    let rutas_publicas = Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/v1/health", get(health_handler))
        .with_state(estado_api);

    let rutas_protegidas = Router::new()
        .merge(rutas_personalizadas)
        .fallback(fallback_404)
        .layer(axum::middleware::from_fn_with_state(
            config.clone(),
            seguridad::middleware_autenticacion_y_usuario,
        ));

    Router::new()
        .merge(rutas_publicas)
        .fallback_service(rutas_protegidas)
        .layer(axum::middleware::from_fn(
            capas::middleware_limite_cuerpo_y_json,
        ))
        .layer(axum::middleware::from_fn_with_state(
            config.clone(),
            capas::middleware_cors,
        ))
        .layer(axum::middleware::from_fn_with_state(
            (estado_rate_limit, config.api.rate_limit_per_minute),
            rate_limit::middleware_rate_limit,
        ))
        .layer(axum::middleware::from_fn(
            capas::middleware_cabeceras_seguridad,
        ))
        .layer(axum::middleware::from_fn(capas::middleware_request_id))
        .layer(axum::middleware::from_fn(capas::middleware_manejo_errores))
}

/// Handle para coordinar el ciclo de vida y apagado limpio del servidor API.
pub struct ServidorApiHandle {
    pub shutdown_tx: Option<oneshot::Sender<()>>,
    pub join_handle: tokio::task::JoinHandle<()>,
    pub port: u16,
}

impl ServidorApiHandle {
    /// Envía la señal de apagado al servidor y espera a que termine de cerrar conexiones.
    pub async fn apagar(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        let _ = self.join_handle.await;
    }
}

/// Inicia el servidor de la API en `127.0.0.1:API_PORT` (si está habilitado en configuración).
pub async fn iniciar_servidor_api(
    config: Arc<nucleo::Config>,
    inicio: Instant,
) -> Result<Option<ServidorApiHandle>, String> {
    let gestor = Arc::new(player::GestorColas::nuevo());
    let ctx = contexto::EstadoApiContexto::nuevo(config, gestor);
    iniciar_servidor_api_con_contexto(ctx, inicio).await
}

/// Inicia el servidor de la API con un contexto explícito (para producción o tests).
pub async fn iniciar_servidor_api_con_contexto(
    ctx: contexto::EstadoApiContexto,
    inicio: Instant,
) -> Result<Option<ServidorApiHandle>, String> {
    if !ctx.config.api.enabled {
        tracing::info!("Servidor API deshabilitado en configuración.");
        return Ok(None);
    }

    let addr = SocketAddr::from(([127, 0, 0, 1], ctx.config.api.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("No se pudo enlazar servidor API en {}: {}", addr, e))?;

    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    tracing::info!("REST API server listening on http://127.0.0.1:{}", port);

    let hub = ctx.sse.clone();
    let app = crear_router_con_contexto(inicio, ctx);
    let (tx, rx) = oneshot::channel();

    let join_handle = tokio::spawn(async move {
        let server = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        );
        let graceful = server.with_graceful_shutdown(async move {
            let _ = rx.await;
            hub.cerrar();
            tracing::info!("Servidor API cerrando conexiones activas...");
        });
        if let Err(e) = graceful.await {
            tracing::error!("Error en ejecución del servidor API: {}", e);
        }
    });

    Ok(Some(ServidorApiHandle {
        shutdown_tx: Some(tx),
        join_handle,
        port,
    }))
}
