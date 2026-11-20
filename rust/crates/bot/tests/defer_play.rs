//! Regresión T6: /play responde con defer antes de la unión de voz y no encola si la unión falla.

use bot::cliente::crear_shard;
use bot::interacciones::RouterInteraccionesReal;
use nucleo::Idiomas;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use twilight_cache_inmemory::InMemoryCache;
use twilight_gateway::Event;
use twilight_model::application::interaction::Interaction;
use twilight_model::gateway::payload::incoming::{ChannelCreate, GuildCreate, VoiceStateUpdate};
use twilight_model::id::Id;

const GUILD: &str = "123456789012345678";
const USER: &str = "987654321098765432";
const BOT: &str = "111111111111111111";
const ADMIN: &str = "222222222222222222";
const CANAL: &str = "333333333333333333";

type Registro = Arc<Mutex<Vec<(String, String)>>>;

fn raiz() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Servidor HTTP local que guarda la línea de petición y el cuerpo de cada llamada.
async fn servidor_mock() -> (String, Registro) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let registro: Registro = Arc::new(Mutex::new(Vec::new()));
    let destino = registro.clone();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let destino = destino.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                loop {
                    let n = socket.read(&mut tmp).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    let Some(fin) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
                        continue;
                    };
                    let cabecera = String::from_utf8_lossy(&buf[..fin]).to_string();
                    let largo = cabecera
                        .lines()
                        .find_map(|l| {
                            let (k, v) = l.split_once(':')?;
                            k.eq_ignore_ascii_case("content-length")
                                .then(|| v.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    if buf.len() < fin + 4 + largo {
                        continue;
                    }
                    let cuerpo =
                        String::from_utf8_lossy(&buf[fin + 4..fin + 4 + largo]).to_string();
                    let linea = cabecera.lines().next().unwrap_or("").to_string();
                    destino.lock().unwrap().push((linea, cuerpo));
                    break;
                }
                let _ = socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}",
                    )
                    .await;
                let _ = socket.shutdown().await;
            });
        }
    });
    (addr, registro)
}

async fn esperar_llamada(registro: &Registro, pista: &str) {
    let limite = Instant::now() + Duration::from_secs(5);
    while Instant::now() < limite {
        if registro
            .lock()
            .unwrap()
            .iter()
            .any(|(l, _)| l.contains(pista))
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("No llegó ninguna llamada con {pista}");
}

fn idiomas() -> Idiomas {
    Idiomas::cargar(&raiz().join("languages"), Some("en")).expect("No se cargaron idiomas")
}

fn contexto(cliente: &str) -> bot::comandos::Contexto {
    let data_dir = raiz().join("data");
    bot::comandos::Contexto {
        idiomas: Arc::new(idiomas()),
        default_language: "en".into(),
        gestor_colas: Arc::new(player::GestorColas::nuevo()),
        almacen_sesiones: Arc::new(player::AlmacenSesiones::nuevo(nucleo::AlmacenJson::new(
            data_dir.join("sessions"),
        ))),
        config_bot: nucleo::BotConfig {
            status: "Testing".into(),
            color: 0x5865F2,
            support_server: "".into(),
            website: "".into(),
            default_language: "en".into(),
            default_volume: 100,
            max_volume: 100,
            max_queue_size: 100,
            max_playlist_size: 100,
            leave_on_empty_ms: 30000,
            leave_on_finish_ms: 30000,
            panel_refresh_ms: 1000,
        },
        command_ids: Arc::new(RwLock::new(HashMap::new())),
        client_id: cliente.into(),
        token: "test".into(),
        root: raiz(),
        data_dir: data_dir.clone(),
        legacy_languages: Arc::new(nucleo::LegacyLanguages::cargar(&raiz())),
        gestor_listas: None,
        gestor_favoritos: None,
        driver: None,
        ytdlp_settings: medios::ytdlp_args::YtDlpSettings::default(),
        ytdlp_args: Vec::new(),
    }
}

/// Caché con servidor, canal de voz, bot con rol administrador y el autor dentro del canal.
fn cache_con_usuario_en_voz() -> Arc<InMemoryCache> {
    let cache = Arc::new(bot::cliente::crear_cache());
    let guild = serde_json::from_value(json!({
        "id": GUILD, "name": "Test", "owner_id": USER, "unavailable": false,
        "afk_timeout": 300, "default_message_notifications": 0, "explicit_content_filter": 0,
        "features": [], "mfa_level": 0, "verification_level": 0, "nsfw_level": 0,
        "preferred_locale": "en-US", "premium_tier": 0, "system_channel_flags": 0,
        "premium_progress_bar_enabled": false, "large": false, "emojis": [], "stickers": [],
        "roles": [
            {"id": GUILD, "name": "@everyone", "permissions": "1024", "color": 0, "colors": {"primary_color": 0},
             "hoist": false, "managed": false, "mentionable": false, "position": 0, "flags": 0},
            {"id": ADMIN, "name": "admin", "permissions": "8", "color": 0, "colors": {"primary_color": 0},
             "hoist": false, "managed": false, "mentionable": false, "position": 1, "flags": 0}
        ],
        "members": [{"user": {"id": BOT, "username": "bot", "discriminator": "0", "avatar": null, "bot": true},
                     "roles": [ADMIN], "deaf": false, "mute": false, "flags": 0, "joined_at": "2024-01-01T00:00:00.000000+00:00"}],
        "channels": [], "voice_states": [], "presences": [], "threads": [], "stage_instances": [],
        "guild_scheduled_events": []
    }))
    .unwrap();
    cache.update(&GuildCreate::Available(guild));
    let canal: ChannelCreate = serde_json::from_value(json!({
        "id": CANAL, "guild_id": GUILD, "type": 2, "name": "General"
    }))
    .unwrap();
    cache.update(&canal);
    let estado: VoiceStateUpdate = serde_json::from_value(json!({
        "guild_id": GUILD, "channel_id": CANAL, "user_id": USER,
        "session_id": "offline", "deaf": false, "mute": false,
        "self_deaf": false, "self_mute": false, "self_video": false, "suppress": false,
        "member": {
            "user": {"id": USER, "username": "listener", "discriminator": "0", "avatar": null, "bot": false},
            "roles": [], "deaf": false, "mute": false, "flags": 0,
            "joined_at": "2024-01-01T00:00:00.000000+00:00"
        }
    }))
    .unwrap();
    cache.update(&Event::VoiceStateUpdate(Box::new(estado)));
    cache
}

fn interaccion() -> Interaction {
    serde_json::from_value(json!({
        "id": "900000000000000001", "application_id": BOT, "type": 2, "token": "token-prueba",
        "guild_id": GUILD, "version": 1, "authorizing_integration_owners": {},
        "member": {
            "user": {"id": USER, "username": "listener", "discriminator": "0", "avatar": null, "bot": false},
            "roles": [], "deaf": false, "mute": false, "flags": 0,
            "joined_at": "2024-01-01T00:00:00.000000+00:00"
        },
        "data": {"id": "600000000000000001", "name": "play", "type": 1,
                 "options": [{"name": "query", "type": 3, "value": "test query"}]}
    }))
    .unwrap()
}

fn router(
    ctx: Arc<bot::comandos::Contexto>,
    url: &str,
    cache: Arc<InMemoryCache>,
    driver: Arc<player::SongbirdDriver>,
) -> Arc<RouterInteraccionesReal> {
    let http = twilight_http::Client::builder()
        .token("token-prueba".into())
        .proxy(url.into(), true)
        .ratelimiter(None)
        .build();
    let mut router = RouterInteraccionesReal::nuevo(ctx, Some(Arc::new(http)));
    router.cache = Some(cache);
    router.driver = Some(driver);
    Arc::new(router)
}

#[tokio::test]
async fn play_responde_con_defer_antes_de_que_termine_la_union() {
    use bot::cliente::RouterInteraccion;
    let (url, registro) = servidor_mock().await;
    // El shard se mantiene vivo: la unión queda esperando VoiceServerUpdate.
    let (_shard, sender) = crear_shard("token-prueba", "prueba");
    let songbird = player::SongbirdDriver::create_songbird(Id::new(1), [(0, sender)].into());
    let driver = Arc::new(player::SongbirdDriver::new(songbird));
    let router = router(
        Arc::new(contexto(BOT)),
        &url,
        cache_con_usuario_en_voz(),
        driver,
    );
    let tarea = tokio::spawn({
        let router = router.clone();
        async move { router.despachar(interaccion()).await }
    });

    esperar_llamada(&registro, "/callback").await;
    assert!(
        !tarea.is_finished(),
        "La unión debía seguir esperando cuando salió el defer"
    );
    {
        let llamadas = registro.lock().unwrap();
        assert_eq!(llamadas.len(), 1, "Solo debe salir el defer: {llamadas:?}");
        assert!(llamadas[0].0.starts_with("POST "), "{:?}", llamadas[0]);
        assert!(llamadas[0].1.contains("\"type\":5"), "{:?}", llamadas[0]);
    }
    tarea.abort();
}

#[tokio::test]
async fn union_fallida_avisa_al_usuario_y_no_encola_la_pista() {
    use bot::cliente::RouterInteraccion;
    let (url, registro) = servidor_mock().await;
    // Shard cerrado: el envío del comando de unión falla al instante.
    let (shard, sender) = crear_shard("token-prueba", "prueba");
    drop(shard);
    let songbird = player::SongbirdDriver::create_songbird(Id::new(1), [(0, sender)].into());
    let driver = Arc::new(player::SongbirdDriver::new(songbird));
    let ctx = Arc::new(contexto(BOT));
    let cola = ctx.gestor_colas.clone();
    let router = router(ctx, &url, cache_con_usuario_en_voz(), driver);

    tokio::time::timeout(Duration::from_secs(10), router.despachar(interaccion()))
        .await
        .expect("El manejador no terminó tras la unión fallida");

    let llamadas = registro.lock().unwrap().clone();
    assert_eq!(llamadas.len(), 2, "Defer y aviso de error: {llamadas:?}");
    assert!(llamadas[0].0.starts_with("POST ") && llamadas[0].1.contains("\"type\":5"));
    assert!(llamadas[1].0.starts_with("PATCH "), "{:?}", llamadas[1]);
    assert!(
        llamadas[1].0.contains("/messages/@original"),
        "{:?}",
        llamadas[1]
    );
    assert!(!llamadas[1].1.is_empty());
    let pistas = cola.obtener_cola(GUILD).await;
    assert!(
        pistas.lock().await.canciones.is_empty(),
        "La pista no debía encolarse"
    );
}
