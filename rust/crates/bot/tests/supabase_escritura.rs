//! Credenciales de Supabase: las escrituras exigen clave de servicio (T4, S-30).

use bot::api::anuncio::ClienteSupabase;
use bot::datos::supabase::{ClienteSupabaseRest, SinCredencialEscritura};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

const LECTURA: &str = "clave-lectura-prueba";
const SERVICIO: &str = "clave-servicio-prueba";
const CREADO: &str = "HTTP/1.1 201 Created\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
const LISTA: &str = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]";

/// Servidor de una sola conexión. Devuelve la URL y las cabeceras que recibió.
fn servidor(respuesta: &'static str) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        loop {
            let n = s.read(&mut tmp).unwrap_or(0);
            buf.extend_from_slice(&tmp[..n]);
            let texto = String::from_utf8_lossy(&buf).to_string();
            if let Some(fin) = texto.find("\r\n\r\n") {
                let len = texto[..fin]
                    .lines()
                    .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
                    .and_then(|l| l.split(':').nth(1))
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if buf.len() >= fin + 4 + len || n == 0 {
                    break;
                }
            } else if n == 0 {
                break;
            }
        }
        let _ = tx.send(String::from_utf8_lossy(&buf).to_string());
        let _ = s.write_all(respuesta.as_bytes());
    });
    (url, rx)
}

#[tokio::test]
async fn escritura_sin_clave_de_servicio_no_abre_conexion() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let cliente = ClienteSupabaseRest::con_claves(url, LECTURA.into(), None);

    assert_eq!(
        cliente.post("rest/v1/song_plays").unwrap_err(),
        SinCredencialEscritura
    );
    assert!(cliente.upsert_bot_status("http://x").await.is_err());
    assert!(
        listener.accept().is_err(),
        "no debe abrirse ninguna conexión"
    );
}

#[tokio::test]
async fn escritura_con_clave_de_servicio_usa_esa_clave() {
    let (url, rx) = servidor(CREADO);
    let cliente = ClienteSupabaseRest::con_claves(url, LECTURA.into(), Some(SERVICIO.into()));

    let res = cliente.upsert_bot_status("http://x").await;
    assert!(res.is_ok(), "{res:?}");
    let cabeceras = rx.recv().unwrap();
    assert!(cabeceras.contains(&format!("Bearer {SERVICIO}")));
    assert!(!cabeceras.contains(LECTURA));
}

#[tokio::test]
async fn lectura_funciona_con_clave_de_lectura_sin_servicio() {
    let (url, rx) = servidor(LISTA);
    let cliente = ClienteSupabaseRest::con_claves(url, LECTURA.into(), None);

    let verificacion = cliente.verify_connection(2000).await;
    assert!(verificacion.ok, "{:?}", verificacion.reason);
    let cabeceras = rx.recv().unwrap();
    assert!(cabeceras.contains(&format!("Bearer {LECTURA}")));
}

#[tokio::test]
async fn el_error_de_escritura_no_contiene_claves() {
    let cliente =
        ClienteSupabaseRest::con_claves("http://127.0.0.1:9".into(), LECTURA.into(), None);

    let msg = cliente.upsert_bot_status("http://x").await.unwrap_err();
    assert!(!msg.contains(LECTURA));
    assert!(!msg.contains(SERVICIO));
}
