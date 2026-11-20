//! Pruebas de orden de búsqueda y candidatos para binarios (Spec S-32).

use bot::binarios::cloudflared::asegurar_cloudflared;
use bot::binarios::ffmpeg::asegurar_ffmpeg;
use bot::binarios::ytdlp::asegurar_ytdlp;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_ytdlp_path_roto_falla_sin_buscar_mas() {
    let dir = tempdir().unwrap();
    let bin_dir = dir.path().join("bin");
    let data_dir = dir.path().join("data");
    let cliente = reqwest::Client::new();

    // Pasar un YTDLP_PATH que no existe ni ejecuta
    let res = asegurar_ytdlp(
        &bin_dir,
        &data_dir,
        "ruta_inexistente_ytdlp.exe",
        None,
        &cliente,
    )
    .await;

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.contains("does not work"),
        "Debe fallar inmediatamente indicando que YTDLP_PATH no funciona: {}",
        err
    );
}

#[tokio::test]
async fn test_ffmpeg_path_roto_falla_sin_buscar_mas() {
    let dir = tempdir().unwrap();
    let bin_dir = dir.path().join("bin");
    let cliente = reqwest::Client::new();

    let res = asegurar_ffmpeg(&bin_dir, "ruta_inexistente_ffmpeg.exe", None, &cliente).await;

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.contains("does not work"),
        "Debe fallar inmediatamente indicando que FFMPEG_PATH no funciona: {}",
        err
    );
}

#[tokio::test]
async fn test_cloudflared_candidato_fuera_de_bin_dir_no_se_borra() {
    let dir = tempdir().unwrap();
    let bin_dir = dir.path().join("bin");
    let root = dir.path().join("root");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&root).unwrap();

    // Crear un cloudflared corrupto en root (fuera de bin_dir)
    let corrupto = root.join("cloudflared.exe");
    fs::write(&corrupto, b"archivo_corrupto_no_ejecutable").unwrap();

    let cliente = reqwest::Client::new();
    let _ = asegurar_cloudflared(
        &bin_dir,
        &root,
        true,
        Some("http://127.0.0.1:9/no_existe.exe"),
        &cliente,
    )
    .await;

    // Verificar que el candidato fuera de bin_dir NO se borró
    assert!(
        corrupto.is_file(),
        "El binario corrupto fuera de bin_dir no debe ser borrado"
    );
}

#[tokio::test]
async fn test_cloudflared_candidato_en_bin_dir_invalido_si_se_borra() {
    let dir = tempdir().unwrap();
    let bin_dir = dir.path().join("bin");
    let root = dir.path().join("root");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&root).unwrap();

    let corrupto_local = bin_dir.join("cloudflared.exe");
    fs::write(&corrupto_local, b"archivo_corrupto_local").unwrap();

    let cliente = reqwest::Client::new();
    let _ = asegurar_cloudflared(
        &bin_dir,
        &root,
        true,
        Some("http://127.0.0.1:9/no_existe.exe"),
        &cliente,
    )
    .await;

    // En bin_dir, la copia corrupta debe ser eliminada
    assert!(
        !corrupto_local.is_file(),
        "La copia corrupta en bin_dir debe ser borrada para reemplazo"
    );
}

#[tokio::test]
async fn test_start_rust_bat_verificacion_de_reglas() {
    // Buscar start-rust.bat en la raíz del proyecto
    let path = std::path::Path::new("../../start-rust.bat");
    let path_alt = std::path::Path::new("start-rust.bat");
    let bat_path = if path.is_file() {
        path
    } else if path_alt.is_file() {
        path_alt
    } else {
        std::path::Path::new("../../../start-rust.bat")
    };

    assert!(
        bat_path.is_file(),
        "start-rust.bat debe existir en la raíz de ignite-music"
    );

    let contenido = fs::read_to_string(bat_path).unwrap();
    assert!(
        contenido.contains("title Beatra"),
        "start-rust.bat debe contener 'title Beatra'"
    );
    assert!(
        contenido.contains(r#"rust\target\release\bot.exe"#),
        "start-rust.bat debe hacer referencia a rust\\target\\release\\bot.exe"
    );
    assert!(
        contenido.contains("cargo build --release -p bot"),
        "start-rust.bat debe sugerir 'cargo build --release -p bot'"
    );
    assert!(
        contenido.contains("timeout /t 5"),
        "start-rust.bat debe esperar 5 segundos tras caída (código 1)"
    );
    assert!(
        contenido.contains("Bot stopped."),
        "start-rust.bat debe mostrar 'Bot stopped.' al salir con código 0"
    );
}
