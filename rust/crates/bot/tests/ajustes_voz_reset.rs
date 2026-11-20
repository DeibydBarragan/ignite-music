//! Reinicio de ajustes de voz: sin clave de servicio no hay HTTP, y un fallo de borrado no rompe el reinicio (M3, S-30).

use bot::datos::ajustes_voz::GestorAjustesVoz;
use bot::datos::supabase::ClienteSupabaseRest;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

const LECTURA: &str = "clave-lectura-prueba";
const SERVICIO: &str = "clave-servicio-prueba";
const BORRADO_OK: &str =
    "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
const BORRADO_ERROR: &str =
    "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

/// Atiende `peticiones` conexiones, una por una, y envía las cabeceras recibidas.
fn servidor(respuesta: &'static str, peticiones: usize) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for _ in 0..peticiones {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = s.read(&mut tmp).unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            let _ = tx.send(String::from_utf8_lossy(&buf).to_string());
            let _ = s.write_all(respuesta.as_bytes());
        }
    });
    (url, rx)
}

fn gestor(cliente: ClienteSupabaseRest) -> GestorAjustesVoz {
    GestorAjustesVoz::nuevo(Some(Arc::new(cliente)), false, "ignite".into(), false)
}

#[tokio::test]
async fn reinicio_sin_clave_de_servicio_no_abre_conexion() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let cliente = ClienteSupabaseRest::con_claves(url, LECTURA.into(), None);

    let def = gestor(cliente).reset_guild_settings("123456").await;

    assert_eq!(def.wake_word, "ignite");
    assert!(def.bindings.is_empty());
    assert!(
        listener.accept().is_err(),
        "no debe abrirse ninguna conexión"
    );
}

#[tokio::test]
async fn reinicio_con_clave_de_servicio_borra_con_esa_clave() {
    let (url, rx) = servidor(BORRADO_OK, 2);
    let cliente = ClienteSupabaseRest::con_claves(url, LECTURA.into(), Some(SERVICIO.into()));

    let def = gestor(cliente).reset_guild_settings("123456").await;

    assert_eq!(def.wake_word, "ignite");
    for _ in 0..2 {
        let cabeceras = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(cabeceras.starts_with("DELETE "), "{cabeceras}");
        assert!(cabeceras.contains(&format!("Bearer {SERVICIO}")));
        assert!(!cabeceras.contains(LECTURA));
    }
}

#[tokio::test]
async fn fallo_de_borrado_http_devuelve_valores_por_defecto() {
    let (url, rx) = servidor(BORRADO_ERROR, 2);
    let cliente = ClienteSupabaseRest::con_claves(url, LECTURA.into(), Some(SERVICIO.into()));

    let def = gestor(cliente).reset_guild_settings("123456").await;

    assert_eq!(def.wake_word, "ignite");
    assert!(def.bindings.is_empty());
    for _ in 0..2 {
        assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok());
    }
}

#[tokio::test]
async fn fallo_de_conexion_al_borrar_no_rompe_el_reinicio() {
    let cliente = ClienteSupabaseRest::con_claves(
        "http://127.0.0.1:9".into(),
        LECTURA.into(),
        Some(SERVICIO.into()),
    );

    let def = gestor(cliente).reset_guild_settings("123456").await;

    assert_eq!(def.wake_word, "ignite");
    assert!(def.bindings.is_empty());
}
