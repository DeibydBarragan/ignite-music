//! Pruebas de descarga con servidor HTTP local (Spec S-32).

use axum::extract::Path as AxumPath;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use bot::binarios::descarga::{descargar_a_fichero, OpcionesDescarga};
use std::net::SocketAddr;
use std::time::Duration;
use tempfile::tempdir;

async fn handler_descarga(AxumPath(caso): AxumPath<String>) -> impl IntoResponse {
    match caso.as_str() {
        "404" => (StatusCode::NOT_FOUND, "Not Found".to_string()).into_response(),
        "pequeno" => (StatusCode::OK, "x".repeat(100)).into_response(),
        "grande" => (StatusCode::OK, "y".repeat(5000)).into_response(),
        "correcto" => (StatusCode::OK, "z".repeat(1024 * 1024)).into_response(),
        "lento" => {
            tokio::time::sleep(Duration::from_millis(500)).await;
            (StatusCode::OK, "lento".repeat(1000)).into_response()
        }
        _ => (StatusCode::BAD_REQUEST, "desconocido".to_string()).into_response(),
    }
}

async fn spawn_test_server() -> String {
    let app = Router::new().route("/descarga/{caso}", get(handler_descarga));
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
async fn test_descarga_404_no_deja_temporal() {
    let server_url = spawn_test_server().await;
    let dir = tempdir().unwrap();
    let destino = dir.path().join("archivo.bin");
    let cliente = reqwest::Client::new();

    let opciones = OpcionesDescarga::default();
    let res = descargar_a_fichero(
        &cliente,
        &format!("{}/descarga/404", server_url),
        &destino,
        opciones,
    )
    .await;

    assert!(res.is_err());
    assert!(res.unwrap_err().contains("HTTP 404"));
    assert!(!destino.exists());

    // Verificar que no quedan archivos temporales en el directorio
    let entradas: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert!(entradas.is_empty(), "No deben quedar temporales");
}

#[tokio::test]
async fn test_descarga_menor_que_minimo_falla() {
    let server_url = spawn_test_server().await;
    let dir = tempdir().unwrap();
    let destino = dir.path().join("pequeno.bin");
    let cliente = reqwest::Client::new();

    let opciones = OpcionesDescarga {
        min_bytes: 1024,
        max_bytes: 10_000,
        sha256_esperado: None,
        timeout: Duration::from_secs(5),
    };
    let res = descargar_a_fichero(
        &cliente,
        &format!("{}/descarga/pequeno", server_url),
        &destino,
        opciones,
    )
    .await;

    assert!(res.is_err());
    assert!(res.unwrap_err().contains("too small"));
    let entradas: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert!(entradas.is_empty());
}

#[tokio::test]
async fn test_descarga_mayor_que_maximo_falla() {
    let server_url = spawn_test_server().await;
    let dir = tempdir().unwrap();
    let destino = dir.path().join("grande.bin");
    let cliente = reqwest::Client::new();

    let opciones = OpcionesDescarga {
        min_bytes: 10,
        max_bytes: 500,
        sha256_esperado: None,
        timeout: Duration::from_secs(5),
    };
    let res = descargar_a_fichero(
        &cliente,
        &format!("{}/descarga/grande", server_url),
        &destino,
        opciones,
    )
    .await;

    assert!(res.is_err());
    assert!(res.unwrap_err().contains("exceeds"));
    let entradas: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert!(entradas.is_empty());
}

#[tokio::test]
async fn test_descarga_sha256_incorrecto_falla() {
    let server_url = spawn_test_server().await;
    let dir = tempdir().unwrap();
    let destino = dir.path().join("sha.bin");
    let cliente = reqwest::Client::new();

    let opciones = OpcionesDescarga {
        min_bytes: 10,
        max_bytes: 2 * 1024 * 1024,
        sha256_esperado: Some(
            "0000000000000000000000000000000000000000000000000000000000000000".into(),
        ),
        timeout: Duration::from_secs(5),
    };
    let res = descargar_a_fichero(
        &cliente,
        &format!("{}/descarga/correcto", server_url),
        &destino,
        opciones,
    )
    .await;

    assert!(res.is_err());
    assert!(res.unwrap_err().contains("Checksum mismatch"));
    let entradas: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert!(entradas.is_empty());
}

#[tokio::test]
async fn test_descarga_timeout_falla() {
    let server_url = spawn_test_server().await;
    let dir = tempdir().unwrap();
    let destino = dir.path().join("lento.bin");
    let cliente = reqwest::Client::new();

    let opciones = OpcionesDescarga {
        min_bytes: 10,
        max_bytes: 50_000,
        sha256_esperado: None,
        timeout: Duration::from_millis(50),
    };
    let res = descargar_a_fichero(
        &cliente,
        &format!("{}/descarga/lento", server_url),
        &destino,
        opciones,
    )
    .await;

    assert!(res.is_err());
    assert!(res.unwrap_err().contains("Tiempo límite superado"));
    let entradas: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert!(entradas.is_empty());
}

#[tokio::test]
async fn test_descarga_correcta_con_sha256() {
    use sha2::{Digest, Sha256};
    let server_url = spawn_test_server().await;
    let dir = tempdir().unwrap();
    let destino = dir.path().join("ok.bin");
    let cliente = reqwest::Client::new();

    let datos = "z".repeat(1024 * 1024);
    let mut hasher = Sha256::new();
    hasher.update(datos.as_bytes());
    let hash_correcto = format!("{:x}", hasher.finalize());

    let opciones = OpcionesDescarga {
        min_bytes: 1024 * 1024,
        max_bytes: 2 * 1024 * 1024,
        sha256_esperado: Some(hash_correcto.clone()),
        timeout: Duration::from_secs(5),
    };

    let res = descargar_a_fichero(
        &cliente,
        &format!("{}/descarga/correcto", server_url),
        &destino,
        opciones,
    )
    .await
    .unwrap();

    assert_eq!(res.bytes, 1024 * 1024);
    assert_eq!(res.sha256, hash_correcto);
    assert!(destino.is_file());
}
