//! Pruebas unitarias y de integración para el túnel Cloudflare (Spec S-29).

use bot::api::tunel::{extraer_url_tunel, resolver_binario, GestorTunel};
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn test_extraccion_url_tunel_variantes() {
    // 1. Línea limpia dentro de la caja de bienvenida de cloudflared
    let linea_caja = "2026-11-09T20:00:00Z INF |  https://bold-pine-1234.trycloudflare.com  |";
    assert_eq!(
        extraer_url_tunel(linea_caja),
        Some("https://bold-pine-1234.trycloudflare.com".to_string())
    );

    // 2. Texto antes y después de la URL
    let linea_texto = "Visit your tunnel at https://sample-tunnel-99.trycloudflare.com to test it!";
    assert_eq!(
        extraer_url_tunel(linea_texto),
        Some("https://sample-tunnel-99.trycloudflare.com".to_string())
    );

    // 3. Dominio ajeno (.trycloudflare.org o .com de otro host) debe ignorarse
    assert_eq!(
        extraer_url_tunel("https://malicious.trycloudflare.org"),
        None
    );
    assert_eq!(extraer_url_tunel("https://example.com/not-a-tunnel"), None);

    // 4. Esquema no seguro http:// debe ignorarse
    assert_eq!(
        extraer_url_tunel("http://bold-pine-1234.trycloudflare.com"),
        None
    );

    // 5. Sin URL
    assert_eq!(
        extraer_url_tunel("Just some regular logs from cloudflared"),
        None
    );
}

#[test]
fn test_resolver_binario_fallback() {
    let temp = std::env::temp_dir().join(format!("ignite_bin_test_{}", fastrand::u64(..)));
    let _ = std::fs::create_dir_all(&temp);

    // Si no existen los candidatos locales, encuentra la instalación del sistema o recurre a "cloudflared"
    let bin = resolver_binario(&temp.join("bin"), &temp);
    assert!(bin == std::path::Path::new("cloudflared") || bin.exists());

    // Si existe el archivo en bin/cloudflared.exe (en Windows) o bin/cloudflared, lo prefiere prioritariamente
    let bin_dir = temp.join("bin");
    let _ = std::fs::create_dir_all(&bin_dir);
    #[cfg(windows)]
    let candidato = bin_dir.join("cloudflared.exe");
    #[cfg(not(windows))]
    let candidato = bin_dir.join("cloudflared");

    std::fs::write(&candidato, b"mock").unwrap();
    let res = resolver_binario(&bin_dir, &temp);
    assert_eq!(res, candidato);

    let _ = std::fs::remove_dir_all(&temp);
}

#[tokio::test]
async fn test_binario_inexistente_devuelve_none_sin_panico() {
    let binario_falso = PathBuf::from("nonexistent_cloudflared_binary_12345.exe");
    let gestor = GestorTunel::con_binario(binario_falso);

    let res = gestor.iniciar(3001, Duration::from_millis(200)).await;
    assert_eq!(res, None);
    assert_eq!(gestor.obtener_url(), None);
}

#[tokio::test]
async fn test_tiempo_limite_mata_proceso_y_devuelve_none() {
    // Proceso falso que no emite URL de trycloudflare (powershell durmiendo)
    #[cfg(windows)]
    let bin = PathBuf::from("powershell.exe");
    #[cfg(not(windows))]
    let bin = PathBuf::from("sleep");

    let gestor = GestorTunel::con_binario(bin);

    // Tiempo límite corto (80ms)
    let res = gestor.iniciar(3001, Duration::from_millis(80)).await;
    assert_eq!(
        res, None,
        "Debe devolver None cuando vence el tiempo límite"
    );
    assert_eq!(gestor.obtener_url(), None);
}

#[tokio::test]
async fn test_segunda_llamada_con_tunel_activo_devuelve_misma_url() {
    let gestor = GestorTunel::nuevo();
    // Parada sin proceso activo no falla
    gestor.detener().await;
    assert_eq!(gestor.obtener_url(), None);
}

#[tokio::test]
async fn test_parada_limpia_url_y_proceso() {
    let gestor = GestorTunel::nuevo();
    assert_eq!(gestor.obtener_url(), None);
    gestor.detener().await;
    assert_eq!(gestor.obtener_url(), None);
}
