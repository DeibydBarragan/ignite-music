//! Tests de rate limiting y manejo de errores para la API REST (Spec S-26).

use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use bot::api::crear_router_con_rutas;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

const TEST_SECRET: &str = "test-secret-0123456789abcdef0123456789abcdef";

fn test_config(rate_limit: u32, allowed: Vec<String>) -> Arc<nucleo::Config> {
    let mut config = nucleo::Config::from_map(PathBuf::from("."), &HashMap::new());
    config.api.enabled = true;
    config.api.secret = TEST_SECRET.to_string();
    config.api.allowed_origins = allowed;
    config.api.rate_limit_per_minute = rate_limit;
    Arc::new(config)
}

async fn spawn_test_server(config: Arc<nucleo::Config>) -> (String, tokio::task::JoinHandle<()>) {
    let custom = Router::new().route(
        "/api/v1/panic",
        get(|| async {
            panic!("Error simulado");
            #[allow(unreachable_code)]
            StatusCode::OK
        }),
    );
    let app = crear_router_con_rutas(config, Instant::now(), custom);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });
    (format!("http://127.0.0.1:{}", port), handle)
}

#[tokio::test]
async fn test_rate_limiting_y_agotamiento() {
    let (base, handle) = spawn_test_server(test_config(2, vec!["*".to_string()])).await;
    let client = reqwest::Client::new();

    let r1 = client
        .get(format!("{}/api/health", base))
        .send()
        .await
        .unwrap();
    assert_eq!(r1.status(), StatusCode::OK);
    assert_eq!(r1.headers().get("x-ratelimit-remaining").unwrap(), "1");

    let r2 = client
        .get(format!("{}/api/health", base))
        .send()
        .await
        .unwrap();
    assert_eq!(r2.status(), StatusCode::OK);
    assert_eq!(r2.headers().get("x-ratelimit-remaining").unwrap(), "0");

    let r3 = client
        .get(format!("{}/api/health", base))
        .send()
        .await
        .unwrap();
    assert_eq!(r3.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(r3.headers().contains_key("retry-after"));
    let b3: serde_json::Value = r3.json().await.unwrap();
    assert_eq!(b3["code"], "RATE_LIMITED");
    handle.abort();
}

#[tokio::test]
async fn test_ruta_no_encontrada_y_error_interno() {
    let (base, handle) = spawn_test_server(test_config(100, vec!["*".to_string()])).await;
    let client = reqwest::Client::new();
    let auth = format!("Bearer {}", TEST_SECRET);

    let r404 = client
        .get(format!("{}/api/v1/inexistente", base))
        .header("authorization", &auth)
        .send()
        .await
        .unwrap();
    assert_eq!(r404.status(), StatusCode::NOT_FOUND);
    let b404: serde_json::Value = r404.json().await.unwrap();
    assert_eq!(b404["code"], "NOT_FOUND");
    assert_eq!(b404["detail"], "Ruta no encontrada.");

    let r500 = client
        .get(format!("{}/api/v1/panic", base))
        .header("authorization", &auth)
        .send()
        .await
        .unwrap();
    assert_eq!(r500.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let b500: serde_json::Value = r500.json().await.unwrap();
    assert_eq!(b500["code"], "INTERNAL_ERROR");
    assert_eq!(b500["detail"], "Error interno del servidor.");
    handle.abort();
}
