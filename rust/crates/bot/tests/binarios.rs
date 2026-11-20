//! Pruebas de integración para el orquestador de binarios y mirrors HTTP (Spec S-32).

use axum::extract::Path as AxumPath;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use bot::binarios::asegurar_binarios;
use bot::binarios::ytdlp::asset_checksum;
use nucleo::Config;
use std::collections::HashMap;
use std::net::SocketAddr;
use tempfile::tempdir;

async fn handler_sums(AxumPath(path): AxumPath<String>) -> impl IntoResponse {
    if path == "con-sums/SHA2-256SUMS" {
        (
            StatusCode::OK,
            "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890  yt-dlp_win.zip\n",
        )
            .into_response()
    } else {
        (StatusCode::NOT_FOUND, "Not found").into_response()
    }
}

async fn spawn_sums_server() -> String {
    let app = Router::new().route("/{*path}", get(handler_sums));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });
    format!("http://127.0.0.1:{}", port)
}

#[tokio::test]
async fn test_asset_checksum_existente_y_inexistente() {
    let server_url = spawn_sums_server().await;
    let cliente = reqwest::Client::new();

    // Base con SHA2-256SUMS presente
    let base_con = format!("{}/con-sums", server_url);
    let hash = asset_checksum("yt-dlp_win.zip", &base_con, &cliente).await;
    assert_eq!(
        hash,
        Some("abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890".to_string())
    );

    // Base sin SHA2-256SUMS (404)
    let base_sin = format!("{}/sin-sums", server_url);
    let hash_sin = asset_checksum("yt-dlp_win.zip", &base_sin, &cliente).await;
    assert_eq!(hash_sin, None);
}

#[tokio::test]
async fn test_asegurar_binarios_falla_con_mensajes_paridad_node() {
    let dir = tempdir().unwrap();
    let bin_dir = dir.path().join("bin");
    let data_dir = dir.path().join("data");
    std::fs::create_dir_all(&bin_dir).unwrap();
    std::fs::create_dir_all(&data_dir).unwrap();

    let mut cfg = Config::from_map(dir.path().to_path_buf(), &HashMap::new());
    cfg.bin_dir = bin_dir;
    cfg.data_dir = data_dir;
    // Forzar rutas inválidas para provocar error de yt-dlp
    cfg.ytdlp.path = "ruta_inexistente_ytdlp_12345.exe".to_string();

    let res = asegurar_binarios(&cfg).await;
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.contains("does not work"),
        "El mensaje debe reflejar que el binario configurado no funciona: {}",
        err
    );
}

#[tokio::test]
async fn test_asegurar_binarios_ffmpeg_invalido_falla() {
    let dir = tempdir().unwrap();
    let bin_dir = dir.path().join("bin");
    let data_dir = dir.path().join("data");
    std::fs::create_dir_all(&bin_dir).unwrap();
    std::fs::create_dir_all(&data_dir).unwrap();

    let mut cfg = Config::from_map(dir.path().to_path_buf(), &HashMap::new());
    cfg.bin_dir = bin_dir;
    cfg.data_dir = data_dir;
    // Forzar yt-dlp inválido para garantizar fallo offline sin tocar red.
    cfg.ytdlp.path = "ruta_inexistente_ytdlp_12345.exe".to_string();
    cfg.ffmpeg_path = "ruta_inexistente_ffmpeg_12345.exe".to_string();

    let res = asegurar_binarios(&cfg).await;
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.contains("does not work"),
        "El mensaje debe indicar que el binario configurado no funciona: {}",
        err
    );
}
#[test]
fn configuracion_ytdlp_compartida_respeta_limite_y_timeout() {
    let mut vars = std::collections::HashMap::new();
    vars.insert("YTDLP_CONCURRENCY".to_string(), "2".to_string());
    vars.insert("YTDLP_TIMEOUT".to_string(), "45".to_string());
    let config = nucleo::Config::from_map(std::path::PathBuf::from("."), &vars);
    let ajustes = bot::binarios::configurar_ytdlp(&config);
    assert_eq!(ajustes.concurrency, 2);
    assert_eq!(ajustes.limiter.limit(), 2);
    assert_eq!(ajustes.timeout_ms, config.ytdlp.timeout_ms);
}
