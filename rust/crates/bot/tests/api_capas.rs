//! Tests de capas HTTP, CORS, rate limiting, límites y ciclo de vida (Spec S-26).

use axum::http::StatusCode;
use axum::routing::post;
use axum::Router;
use bot::api::{crear_router_con_rutas, MAX_BODY_BYTES};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

const TEST_SECRET: &str = "test-secret-0123456789abcdef0123456789abcdef";
const USER_ID: &str = "123456789012345678";

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
        "/api/v1/echo",
        post(|b: String| async move { (StatusCode::OK, b) }),
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
async fn test_salud_y_cabeceras_de_seguridad() {
    let (base, handle) = spawn_test_server(test_config(100, vec!["*".to_string()])).await;
    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/api/health", base))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let h = res.headers();
    assert!(h.contains_key("content-security-policy"));
    let esperadas = [
        ("cross-origin-opener-policy", "same-origin"),
        ("cross-origin-resource-policy", "same-origin"),
        ("origin-agent-cluster", "?1"),
        ("referrer-policy", "no-referrer"),
        (
            "strict-transport-security",
            "max-age=31536000; includeSubDomains",
        ),
        ("x-content-type-options", "nosniff"),
        ("x-dns-prefetch-control", "off"),
        ("x-download-options", "noopen"),
        ("x-frame-options", "SAMEORIGIN"),
        ("x-permitted-cross-domain-policies", "none"),
        ("x-xss-protection", "0"),
    ];
    for (k, v) in esperadas {
        assert_eq!(h.get(k).unwrap(), v);
    }
    assert!(!h.contains_key("x-powered-by"));

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert!(body["uptime"].is_number());
    handle.abort();
}

#[tokio::test]
async fn test_cors_origenes_y_preflight_options() {
    let (base, handle) =
        spawn_test_server(test_config(100, vec!["http://frontend.local".to_string()])).await;
    let client = reqwest::Client::new();

    let r1 = client
        .get(format!("{}/api/health", base))
        .header("origin", "http://frontend.local")
        .send()
        .await
        .unwrap();
    assert_eq!(
        r1.headers().get("access-control-allow-origin").unwrap(),
        "http://frontend.local"
    );

    let r2 = client
        .get(format!("{}/api/health", base))
        .header("origin", "http://evil.com")
        .send()
        .await
        .unwrap();
    assert!(!r2.headers().contains_key("access-control-allow-origin"));

    let r3 = client
        .get(format!("{}/api/health", base))
        .send()
        .await
        .unwrap();
    assert!(!r3.headers().contains_key("access-control-allow-origin"));

    let r_options = client
        .request(reqwest::Method::OPTIONS, format!("{}/api/v1/echo", base))
        .header("origin", "http://frontend.local")
        .header("access-control-request-method", "POST")
        .header(
            "access-control-request-headers",
            "authorization, content-type, x-user-discord-id",
        )
        .send()
        .await
        .unwrap();
    assert_eq!(r_options.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        r_options
            .headers()
            .get("access-control-allow-origin")
            .unwrap(),
        "http://frontend.local"
    );
    assert!(r_options
        .headers()
        .contains_key("access-control-allow-methods"));
    handle.abort();
}

#[tokio::test]
async fn test_limite_cuerpo_y_json_invalido() {
    let (base, handle) = spawn_test_server(test_config(100, vec!["*".to_string()])).await;
    let client = reqwest::Client::new();
    let auth = format!("Bearer {}", TEST_SECRET);

    // Más de 100 kB (100 kB + 1 byte) -> 413
    let r1 = client
        .post(format!("{}/api/v1/echo", base))
        .header("authorization", &auth)
        .header("x-user-discord-id", USER_ID)
        .body("x".repeat(MAX_BODY_BYTES + 1))
        .send()
        .await
        .unwrap();
    assert_eq!(r1.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let b1: serde_json::Value = r1.json().await.unwrap();
    assert_eq!(b1["code"], "PAYLOAD_TOO_LARGE");

    // JSON mal formado -> 400
    let r2 = client
        .post(format!("{}/api/v1/echo", base))
        .header("authorization", &auth)
        .header("x-user-discord-id", USER_ID)
        .header("content-type", "application/json")
        .body("{ malformed json")
        .send()
        .await
        .unwrap();
    assert_eq!(r2.status(), StatusCode::BAD_REQUEST);
    let b2: serde_json::Value = r2.json().await.unwrap();
    assert_eq!(b2["code"], "INVALID_JSON");

    // JSON válido pasa
    let r3 = client
        .post(format!("{}/api/v1/echo", base))
        .header("authorization", &auth)
        .header("x-user-discord-id", USER_ID)
        .header("content-type", "application/json")
        .body("{\"ok\":true}")
        .send()
        .await
        .unwrap();
    assert_eq!(r3.status(), StatusCode::OK);
    assert_eq!(r3.text().await.unwrap(), "{\"ok\":true}");
    handle.abort();
}
