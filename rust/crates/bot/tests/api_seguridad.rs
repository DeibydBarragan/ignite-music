//! Tests de seguridad y autenticación para la API REST (Spec S-26).

use axum::routing::{get, post};
use axum::Router;
use bot::api::seguridad::{
    es_metodo_mutacion, es_snowflake_valido, extract_bearer, safe_equal, UsuarioDiscord,
};
use bot::api::{crear_router_con_rutas, MIN_SECRET_LENGTH};
use reqwest::StatusCode;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

const TEST_SECRET: &str = "test-secret-0123456789abcdef0123456789abcdef";

fn test_config(secret: &str) -> Arc<nucleo::Config> {
    let mut config = nucleo::Config::from_map(PathBuf::from("."), &HashMap::new());
    config.api.enabled = true;
    config.api.secret = secret.to_string();
    config.api.allowed_origins = vec!["http://localhost:3000".to_string()];
    config.api.rate_limit_per_minute = 1000;
    Arc::new(config)
}

async fn spawn_test_server(config: Arc<nucleo::Config>) -> (String, tokio::task::JoinHandle<()>) {
    let custom_routes = Router::new()
        .route(
            "/api/v1/test-mutation",
            post(|user: Option<axum::Extension<UsuarioDiscord>>| async move {
                (StatusCode::OK, user.map(|u| u.0 .0).unwrap_or_default())
            }),
        )
        .route("/api/v1/test-get", get(|| async { (StatusCode::OK, "ok") }));

    let app = crear_router_con_rutas(config, Instant::now(), custom_routes);
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

#[test]
fn test_safe_equal_unitario() {
    assert!(safe_equal(TEST_SECRET, TEST_SECRET));
    assert!(!safe_equal(TEST_SECRET, "short"));
    assert!(!safe_equal(TEST_SECRET, &"x".repeat(TEST_SECRET.len())));
    assert!(safe_equal("", ""));
    assert!(!safe_equal("a", "b"));
}

#[test]
fn test_extract_bearer_unitario() {
    assert_eq!(extract_bearer("Bearer token123"), Some("token123"));
    assert_eq!(extract_bearer("bearer token123"), Some("token123"));
    assert_eq!(extract_bearer("BEARER token123"), Some("token123"));
    assert_eq!(extract_bearer("   Bearer   token123   "), Some("token123"));
    assert_eq!(extract_bearer("Basic token123"), None);
    assert_eq!(extract_bearer("Bearer "), None);
    assert_eq!(extract_bearer("Bearer token with spaces"), None);
    assert_eq!(extract_bearer(""), None);
}

#[test]
fn test_es_snowflake_valido_unitario() {
    assert!(!es_snowflake_valido("1234567890123456")); // 16
    assert!(es_snowflake_valido("12345678901234567")); // 17
    assert!(es_snowflake_valido("12345678901234567890")); // 20
    assert!(!es_snowflake_valido("123456789012345678901")); // 21
    assert!(!es_snowflake_valido("12345678901234567a"));
}

#[test]
fn test_es_metodo_mutacion_unitario() {
    use axum::http::Method;
    assert!(es_metodo_mutacion(&Method::POST));
    assert!(es_metodo_mutacion(&Method::PUT));
    assert!(es_metodo_mutacion(&Method::PATCH));
    assert!(es_metodo_mutacion(&Method::DELETE));
    assert!(!es_metodo_mutacion(&Method::GET));
    assert!(!es_metodo_mutacion(&Method::OPTIONS));
}

#[tokio::test]
async fn test_api_secret_corto_responde_503_salvo_salud() {
    let short_secret = "a".repeat(MIN_SECRET_LENGTH - 1);
    let (base, handle) = spawn_test_server(test_config(&short_secret)).await;
    let client = reqwest::Client::new();

    // Salud responde 200 sin importar el secreto
    let r_health = client
        .get(format!("{}/api/health", base))
        .send()
        .await
        .unwrap();
    assert_eq!(r_health.status(), StatusCode::OK);
    let r_health_v1 = client
        .get(format!("{}/api/v1/health", base))
        .send()
        .await
        .unwrap();
    assert_eq!(r_health_v1.status(), StatusCode::OK);

    // Cualquier otra ruta protegida responde 503 AUTH_NOT_CONFIGURED
    let res = client
        .get(format!("{}/api/v1/test-get", base))
        .header("authorization", format!("Bearer {}", short_secret))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["code"], "AUTH_NOT_CONFIGURED");
    assert_eq!(body["status"], 503);

    handle.abort();
}

#[tokio::test]
async fn test_autenticacion_bearer_variantes() {
    let (base, handle) = spawn_test_server(test_config(TEST_SECRET)).await;
    let client = reqwest::Client::new();

    // Sin cabecera
    let r1 = client
        .get(format!("{}/api/v1/test-get", base))
        .send()
        .await
        .unwrap();
    assert_eq!(r1.status(), StatusCode::UNAUTHORIZED);
    let b1: serde_json::Value = r1.json().await.unwrap();
    assert_eq!(b1["code"], "UNAUTHORIZED");

    // Esquema distinto (Basic)
    let r2 = client
        .get(format!("{}/api/v1/test-get", base))
        .header("authorization", "Basic 12345")
        .send()
        .await
        .unwrap();
    assert_eq!(r2.status(), StatusCode::UNAUTHORIZED);

    // Token longitud errónea
    let r3 = client
        .get(format!("{}/api/v1/test-get", base))
        .header("authorization", "Bearer short")
        .send()
        .await
        .unwrap();
    assert_eq!(r3.status(), StatusCode::UNAUTHORIZED);

    // Token misma longitud pero diferente
    let r4 = client
        .get(format!("{}/api/v1/test-get", base))
        .header(
            "authorization",
            format!("Bearer {}", "x".repeat(TEST_SECRET.len())),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(r4.status(), StatusCode::UNAUTHORIZED);

    // Minúsculas bearer
    let r5 = client
        .get(format!("{}/api/v1/test-get", base))
        .header("authorization", format!("bearer {}", TEST_SECRET))
        .send()
        .await
        .unwrap();
    assert_eq!(r5.status(), StatusCode::OK);

    // Espacios extra
    let r6 = client
        .get(format!("{}/api/v1/test-get", base))
        .header("authorization", format!("  Bearer   {}  ", TEST_SECRET))
        .send()
        .await
        .unwrap();
    assert_eq!(r6.status(), StatusCode::OK);

    handle.abort();
}

#[tokio::test]
async fn test_mutaciones_requieren_snowflake_valido() {
    let (base, handle) = spawn_test_server(test_config(TEST_SECRET)).await;
    let client = reqwest::Client::new();
    let auth = format!("Bearer {}", TEST_SECRET);

    // Mutación sin cabecera X-User-Discord-Id -> 400
    let r1 = client
        .post(format!("{}/api/v1/test-mutation", base))
        .header("authorization", &auth)
        .send()
        .await
        .unwrap();
    assert_eq!(r1.status(), StatusCode::BAD_REQUEST);
    let b1: serde_json::Value = r1.json().await.unwrap();
    assert_eq!(b1["code"], "INVALID_USER");

    // Mutación con 16 dígitos -> 400
    let r2 = client
        .post(format!("{}/api/v1/test-mutation", base))
        .header("authorization", &auth)
        .header("x-user-discord-id", "1234567890123456")
        .send()
        .await
        .unwrap();
    assert_eq!(r2.status(), StatusCode::BAD_REQUEST);

    // Mutación con 17 dígitos -> 200 y usuario capturado
    let r3 = client
        .post(format!("{}/api/v1/test-mutation", base))
        .header("authorization", &auth)
        .header("x-user-discord-id", "12345678901234567")
        .send()
        .await
        .unwrap();
    assert_eq!(r3.status(), StatusCode::OK);
    assert_eq!(r3.text().await.unwrap(), "12345678901234567");

    // Mutación con 20 dígitos -> 200 y usuario capturado
    let r4 = client
        .post(format!("{}/api/v1/test-mutation", base))
        .header("authorization", &auth)
        .header("x-user-discord-id", "12345678901234567890")
        .send()
        .await
        .unwrap();
    assert_eq!(r4.status(), StatusCode::OK);
    assert_eq!(r4.text().await.unwrap(), "12345678901234567890");

    // GET no exige X-User-Discord-Id
    let r5 = client
        .get(format!("{}/api/v1/test-get", base))
        .header("authorization", &auth)
        .send()
        .await
        .unwrap();
    assert_eq!(r5.status(), StatusCode::OK);
    handle.abort();
}
