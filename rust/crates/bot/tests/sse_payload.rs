//! Paridad del payload SSE con GET /player, latido de player.sync y apagado con cliente parado (Spec S-28).

use bot::api::ciclo_vida::apagar_servidor_acotado;
use bot::api::contexto::EstadoApiContexto;
use bot::api::crear_router_con_contexto;
use bot::api::iniciar_servidor_api_con_contexto;
use futures_util::StreamExt;
use nucleo::Config;
use player::cola::ElementoCola;
use player::GestorColas;
use reqwest::Client;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

const SECRET: &str = "test-secret-0123456789abcdef0123456789abcdef";
const GUILD: &str = "123456789012345678";
const USER: &str = "345678901234567890";
const VOICE: &str = "234567890123456789";

fn contexto_prueba() -> EstadoApiContexto {
    let mut cfg = Config::from_map(
        std::path::PathBuf::from("."),
        &std::collections::HashMap::new(),
    );
    cfg.api.secret = SECRET.to_string();
    cfg.api.enabled = true;
    cfg.api.port = 0;
    cfg.data_dir = std::env::temp_dir().join(format!("ignite_sse_payload_{}", fastrand::u64(..)));
    EstadoApiContexto::nuevo(Arc::new(cfg), Arc::new(GestorColas::nuevo()))
}

async fn encolar_cancion(ctx: &EstadoApiContexto, nombre: &str) {
    let cancion = medios::Cancion {
        fuente: "youtube".to_string(),
        id: nombre.to_string(),
        nombre: nombre.to_string(),
        artista: "Artista".to_string(),
        url: format!("https://example.com/{nombre}"),
        duracion: 213,
        stream_url: None,
        es_directo: false,
        miniatura: None,
        autor_url: None,
        edad_restringida: false,
        es_letra: false,
    };
    let cola = ctx.gestor_colas.obtener_cola(GUILD).await;
    let mut c = cola.lock().await;
    c.canal_voz = Some(VOICE.to_string());
    c.canciones
        .push(ElementoCola::nuevo(cancion, Some(USER.to_string())));
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

/// Lee del stream el siguiente bloque `player.sync` y devuelve su `data` como JSON.
async fn siguiente_sync(res: &mut reqwest::Response, buf: &mut String) -> String {
    loop {
        if let Some(pos) = buf.find("\n\n") {
            let bloque: String = buf.drain(..pos + 2).collect();
            if bloque.starts_with("event: player.sync\n") {
                let data = bloque
                    .lines()
                    .find_map(|l| l.strip_prefix("data: "))
                    .expect("bloque player.sync sin data")
                    .to_string();
                return data;
            }
            continue;
        }
        let chunk = res.chunk().await.unwrap().expect("stream abierto");
        buf.push_str(&String::from_utf8_lossy(&chunk));
    }
}

async fn get_player(client: &Client, base: &str) -> Value {
    client
        .get(format!("{base}/api/v1/guilds/{GUILD}/player"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn evento_inicial_es_igual_al_cuerpo_de_get_player() {
    let ctx = contexto_prueba();
    encolar_cancion(&ctx, "uno").await;
    encolar_cancion(&ctx, "dos").await;
    let (base, _h) = spawn_api(ctx).await;
    let client = Client::new();

    let mut res = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/events"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    let mut buf = String::new();
    let inicial: Value = serde_json::from_str(&siguiente_sync(&mut res, &mut buf).await).unwrap();

    let get = get_player(&client, &base).await;
    assert_eq!(inicial, get);
    assert_eq!(get["queue"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn player_sync_tras_un_cambio_es_igual_a_get_player() {
    let ctx = contexto_prueba();
    let hub = ctx.sse.clone();
    let (base, _h) = spawn_api(ctx.clone()).await;
    let client = Client::new();

    let mut res = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/events"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    let mut buf = String::new();
    siguiente_sync(&mut res, &mut buf).await;

    encolar_cancion(&ctx, "tres").await;
    assert_eq!(hub.publicar_sincronizacion(GUILD, &ctx).await, 1);

    let sync: Value = serde_json::from_str(&siguiente_sync(&mut res, &mut buf).await).unwrap();
    let get = get_player(&client, &base).await;
    assert_eq!(sync, get);
    assert_eq!(get["queue"].as_array().unwrap().len(), 0);
    assert_eq!(get["currentSong"]["title"], "tres");
}

#[tokio::test]
async fn sincronizacion_nunca_emite_cuerpo_vacio() {
    let ctx = contexto_prueba();
    let hub = ctx.sse.clone();
    let (_id, mut stream) = hub.suscribir(GUILD);

    hub.publicar_sincronizacion(GUILD, &ctx).await;
    encolar_cancion(&ctx, "uno").await;
    hub.publicar_sincronizacion(GUILD, &ctx).await;

    let _connected = stream.next().await.unwrap().unwrap();
    for _ in 0..2 {
        let bytes = stream.next().await.unwrap().unwrap();
        let texto = String::from_utf8(bytes.to_vec()).unwrap();
        let data = texto
            .strip_prefix("event: player.sync\ndata: ")
            .and_then(|s| s.strip_suffix("\n\n"))
            .expect("formato player.sync");
        let valor: Value = serde_json::from_str(data).unwrap();
        assert!(valor.is_object(), "cuerpo no vacio y objeto JSON");
        assert!(valor.get("hasQueue").is_some());
    }
}

#[tokio::test]
async fn apagado_con_cliente_sse_parado_termina_antes_de_10_s() {
    let ctx = contexto_prueba();
    let hub = ctx.sse.clone();
    let srv = iniciar_servidor_api_con_contexto(ctx, Instant::now())
        .await
        .unwrap()
        .expect("servidor API activo");

    let mut socket = tokio::net::TcpStream::connect(("127.0.0.1", srv.port))
        .await
        .unwrap();
    let peticion = format!(
        "GET /api/v1/guilds/{GUILD}/events HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {SECRET}\r\n\r\n"
    );
    socket.write_all(peticion.as_bytes()).await.unwrap();

    for _ in 0..200 {
        if hub.size(Some(GUILD)) == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(hub.size(Some(GUILD)), 1, "el cliente debe quedar suscrito");

    // El cliente no lee. Con 80 eventos de 256 KiB, el búfer del socket se llena.
    let relleno = "x".repeat(256 * 1024);
    for i in 0..80 {
        hub.publicar(GUILD, "player.sync", &json!({ "i": i, "relleno": relleno }));
    }

    // Mismo orden que arranque.rs: primero se cierra el hub SSE, después el servidor.
    let inicio = Instant::now();
    hub.cerrar();
    tokio::time::timeout(Duration::from_secs(20), apagar_servidor_acotado(srv))
        .await
        .expect("el apagado no debe quedar bloqueado por un cliente parado");
    assert!(
        inicio.elapsed() < Duration::from_secs(10),
        "el apagado tardó {:?}",
        inicio.elapsed()
    );
}
