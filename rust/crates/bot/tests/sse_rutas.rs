//! Pruebas de integración para la ruta SSE /events (Spec S-28).

use bot::api::contexto::{EstadoApiContexto, ProveedorApi};
use bot::api::crear_router_con_contexto;
use bot::api::sse::HubSse;
use futures_util::StreamExt;
use nucleo::Config;
use player::GestorColas;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

const SECRET: &str = "test-secret-0123456789abcdef0123456789abcdef";
const GUILD: &str = "123456789012345678";
const USER: &str = "987654321098765432";

fn test_context() -> EstadoApiContexto {
    let mut cfg = Config::from_map(
        std::path::PathBuf::from("."),
        &std::collections::HashMap::new(),
    );
    cfg.api.secret = SECRET.to_string();
    cfg.api.enabled = true;
    cfg.data_dir = std::env::temp_dir().join(format!("ignite_sse_test_{}", fastrand::u64(..)));
    let config = Arc::new(cfg);
    let gestor = Arc::new(GestorColas::nuevo());
    EstadoApiContexto::nuevo(config, gestor)
}

async fn spawn_api(ctx: EstadoApiContexto) -> (String, tokio::task::JoinHandle<()>) {
    let app = crear_router_con_contexto(Instant::now(), ctx);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });
    (format!("http://127.0.0.1:{port}"), handle)
}

struct GuildNotFoundProvider;
#[async_trait::async_trait]
impl ProveedorApi for GuildNotFoundProvider {
    async fn verificar_servidor(&self, _guild_id: &str) -> Result<(), (u16, &'static str, String)> {
        Err((
            404,
            "guild_not_found",
            "The bot is not in this guild".to_string(),
        ))
    }
}

#[tokio::test]
async fn test_sse_ruta_cabeceras_y_estado_inicial() {
    let (base, _h) = spawn_api(test_context()).await;
    let client = Client::new();

    let mut res = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/events"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let headers = res.headers();
    assert_eq!(
        headers.get("content-type").unwrap(),
        "text/event-stream; charset=utf-8"
    );
    assert_eq!(
        headers.get("cache-control").unwrap(),
        "no-cache, no-transform"
    );
    assert_eq!(headers.get("connection").unwrap(), "keep-alive");
    assert_eq!(headers.get("x-accel-buffering").unwrap(), "no");

    let chunk1 = res.chunk().await.unwrap().expect("chunk1");
    let text1 = String::from_utf8_lossy(&chunk1);
    assert_eq!(text1, ": connected\n\n");

    let chunk2 = res.chunk().await.unwrap().expect("chunk2");
    let text2 = String::from_utf8_lossy(&chunk2);
    assert!(text2.starts_with("event: player.sync\ndata: {"));
    let initial: Value = serde_json::from_str(
        text2
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap(),
    )
    .unwrap();
    let player: Value = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/player"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(initial, player);
}

#[tokio::test]
async fn test_sse_ruta_estado_inicial_con_error_404_sin_stream() {
    let mut ctx = test_context();
    ctx.proveedor = Arc::new(GuildNotFoundProvider);
    let (base, _h) = spawn_api(ctx).await;
    let client = Client::new();

    let res = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/events"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        res.headers().get("content-type").unwrap(),
        "application/problem+json; charset=utf-8"
    );
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["code"], "guild_not_found");
    assert_eq!(body["status"], 404);
}

#[tokio::test]
async fn test_sse_ruta_snowflake_invalido_400_sin_stream() {
    let (base, _h) = spawn_api(test_context()).await;
    let client = Client::new();

    let res = client
        .get(format!("{base}/api/v1/guilds/no-snowflake/events"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        res.headers().get("content-type").unwrap(),
        "application/problem+json; charset=utf-8"
    );
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["code"], "INVALID_SNOWFLAKE");
    assert_eq!(body["status"], 400);
}

#[tokio::test]
async fn test_sse_control_mutaciones_y_orden_de_eventos() {
    let (base, _h) = spawn_api(test_context()).await;
    let client = Client::new();

    // 1. Abrir suscripción SSE
    let mut res = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/events"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Drenar los 2 chunks iniciales (: connected y el initial player.sync)
    let c1 = res.chunk().await.unwrap().unwrap();
    assert_eq!(String::from_utf8_lossy(&c1), ": connected\n\n");
    let c2 = res.chunk().await.unwrap().unwrap();
    assert!(String::from_utf8_lossy(&c2).starts_with("event: player.sync"));

    // 2. Añadir una canción para que haya cola activa
    let play_res = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/play"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({ "query": "https://www.youtube.com/watch?v=dQw4w9WgXcQ" }))
        .send()
        .await
        .unwrap();
    assert_eq!(play_res.status(), StatusCode::OK);

    // Consumir el evento player.sync emitido por /play
    let play_event = res.chunk().await.unwrap().expect("evento play");
    assert!(String::from_utf8_lossy(&play_event).contains("event: player.sync"));
    let text = String::from_utf8_lossy(&play_event);
    let snapshot: Value = serde_json::from_str(
        text.lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap(),
    )
    .unwrap();
    let player: Value = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/player"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(snapshot, player);

    // 3. Ejecutar 10 cambios seguidos (10 niveles de volumen distintos)
    for i in 1..=10 {
        let control_res = client
            .post(format!("{base}/api/v1/guilds/{GUILD}/control"))
            .bearer_auth(SECRET)
            .header("x-user-discord-id", USER)
            .json(&json!({ "action": "volume", "value": i * 10 }))
            .send()
            .await
            .unwrap();
        assert_eq!(control_res.status(), StatusCode::OK);
    }

    // 4. Verificar que los 10 cambios llegan en orden exacto
    for i in 1..=10 {
        let chunk = res.chunk().await.unwrap().expect("evento sse");
        let text = String::from_utf8_lossy(&chunk);
        assert!(text.contains("event: player.sync"));
        let target = format!("\"volume\":{}", i * 10);
        assert!(
            text.contains(&target),
            "Esperaba {target} en orden, recibido: {text}"
        );
    }
}

#[tokio::test]
async fn test_sse_cerrar_servidor_especifico() {
    let hub = HubSse::nuevo(Duration::ZERO);
    let (_id1, mut stream_a) = hub.suscribir("888888888888888888");
    let (_id2, mut stream_b) = hub.suscribir("999999999999999999");

    let _ = stream_a.next().await;
    let _ = stream_b.next().await;

    hub.cerrar_servidor("888888888888888888");

    assert!(stream_a.next().await.is_none());
    assert_eq!(hub.size(Some("999999999999999999")), 1);

    hub.publicar("999999999999999999", "ping", &json!({}));
    assert!(stream_b.next().await.is_some());

    hub.cerrar();
}

#[tokio::test]
async fn test_sse_publicar_voz_log() {
    let hub = HubSse::nuevo(Duration::ZERO);
    let gid = "101010101010101010";

    let log_data = json!({ "id": "vlog_123", "commandId": "pause", "status": "executed" });

    assert_eq!(hub.publicar_voz_log(gid, &log_data), 0);

    let (_id, mut stream) = hub.suscribir(gid);
    let _ = stream.next().await;

    assert_eq!(hub.publicar_voz_log(gid, &log_data), 1);
    let ev = stream.next().await.unwrap().unwrap();
    let text = String::from_utf8_lossy(&ev);
    assert!(text.starts_with("event: voice.log\ndata: {"));
    assert!(text.contains("\"commandId\":\"pause\""));

    hub.cerrar();
}
