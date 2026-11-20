//! POST /play por el handler real: el historial se registra una vez por arranque y un fallo del
//! historial no detiene la reproducción. Los dos puntos de inyección del proveedor sustituyen la
//! resolución por yt-dlp y la unión a voz de Discord; el resto del camino es el real.

use crate::api::contexto::{EstadoApiContexto, GestorDevMode};
use crate::api::eventos_reproduccion::{conectar, RegistroHistorial};
use crate::api::proveedor_real::ProveedorApiReal;
use crate::api::rutas::reproductor_play::{post_play, PlayBody};
use crate::api::sse::HubSse;
use crate::datos::historial::RegistroCancionInput;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::Json;
use medios::modelos::ResultadoResolucion;
use medios::ytdlp_args::YtDlpSettings;
use nucleo::Config;
use player::GestorColas;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use twilight_cache_inmemory::InMemoryCache;
use twilight_gateway::Event;
use twilight_model::gateway::payload::incoming::{ChannelCreate, GuildCreate, VoiceStateUpdate};
use twilight_model::id::Id;

const GUILD: &str = "123456789012345678";
const GUILD_U64: u64 = 123456789012345678;
const USER: &str = "987654321098765432";
const BOT: &str = "111111111111111111";
const BOT_U64: u64 = 111111111111111111;
const ADMIN: &str = "222222222222222222";
const CANAL: &str = "333333333333333333";

/// Doble del registrador: guarda el título por llamada y puede fallar a propósito.
#[derive(Default)]
struct RegistradorFalso {
    llamadas: Mutex<Vec<String>>,
    falla: bool,
}

impl RegistroHistorial for RegistradorFalso {
    async fn registrar(&self, _guild_id: &str, input: &RegistroCancionInput) -> Result<(), String> {
        self.llamadas
            .lock()
            .unwrap()
            .push(input.title.clone().unwrap_or_default());
        if self.falla {
            Err("fallo simulado".into())
        } else {
            Ok(())
        }
    }
}

fn cancion(nombre: &str) -> medios::Cancion {
    medios::Cancion {
        fuente: "youtube".into(),
        id: nombre.into(),
        nombre: nombre.into(),
        artista: "Artista".into(),
        url: format!("https://example.invalid/watch?v={nombre}"),
        stream_url: None,
        es_directo: false,
        duracion: 180,
        miniatura: None,
        autor_url: None,
        edad_restringida: false,
        es_letra: false,
    }
}

/// Caché con servidor, canal de voz y el autor dentro del canal (la ruta de /play lo necesita).
fn cache_con_usuario_en_voz() -> InMemoryCache {
    let cache = crate::cliente::crear_cache();
    let guild: twilight_model::guild::Guild = serde_json::from_value(json!({
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

/// Estado de la API con el proveedor real, driver de Songbird sin Discord y el listener conectado.
async fn montar(
    registrador: Arc<RegistradorFalso>,
) -> (
    Arc<player::SongbirdDriver>,
    Arc<ProveedorApiReal>,
    EstadoApiContexto,
) {
    let shard = twilight_gateway::Shard::new(
        twilight_gateway::ShardId::ONE,
        "token-de-prueba".into(),
        twilight_gateway::Intents::GUILD_VOICE_STATES,
    );
    let songbird =
        player::SongbirdDriver::create_songbird(Id::new(1), [(0, shard.sender())].into());
    drop(shard);
    let driver = Arc::new(player::SongbirdDriver::new(songbird));
    driver.get_or_insert(GUILD_U64).unwrap();
    let colas = Arc::new(GestorColas::nuevo());
    let mut prov = ProveedorApiReal::nuevo(
        Arc::new(cache_con_usuario_en_voz()),
        None,
        Some(driver.clone()),
        colas.clone(),
        None,
        BOT_U64,
        YtDlpSettings::default(),
        Vec::new(),
        None,
        100,
        100,
    );
    prov.prueba_sin_union = true;
    prov.prueba_resolucion = Some(Arc::new(|| ResultadoResolucion::Cancion(cancion("A"))));
    let prov = Arc::new(prov);
    let hub = HubSse::por_defecto();
    conectar(
        &prov,
        &hub,
        &colas,
        "yt-dlp",
        &[],
        &YtDlpSettings::default(),
        &[],
        Some(registrador),
        Arc::new(GestorDevMode::nuevo("no-existe.json".into())),
        Vec::new(),
    );
    let mut cfg = Config::from_map(std::path::PathBuf::from("."), &HashMap::new());
    cfg.data_dir = std::env::temp_dir().join("ignite_post_play_historial");
    let ctx =
        EstadoApiContexto::nuevo_con_sse(Arc::new(cfg), colas, hub).con_proveedor(prov.clone());
    // El listener solo guarda un Weak del proveedor: este Arc debe vivir hasta el final de la prueba.
    (driver, prov, ctx)
}

async fn post_play_de_prueba(ctx: EstadoApiContexto) -> Response {
    post_play(
        State(ctx),
        Path(GUILD.to_string()),
        HeaderMap::new(),
        None,
        Json(PlayBody {
            query: Some("cancion de prueba".into()),
            voice_channel_id: Some(CANAL.into()),
            user_id: Some(USER.into()),
            next: None,
            skip: None,
        }),
    )
    .await
}

/// El listener corre en una tarea aparte: espera a `n` llamadas y deja pasar un margen para detectar extras.
async fn esperar(registrador: &RegistradorFalso, n: usize) {
    for _ in 0..100 {
        if registrador.llamadas.lock().unwrap().len() >= n {
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(registrador.llamadas.lock().unwrap().len(), n);
}

#[tokio::test]
async fn post_play_registra_historial_una_vez() {
    let reg = Arc::new(RegistradorFalso::default());
    let (driver, _prov, ctx) = montar(reg.clone()).await;
    let respuesta = post_play_de_prueba(ctx).await;
    assert_eq!(respuesta.status(), StatusCode::OK);
    let cuerpo = axum::body::to_bytes(respuesta.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&cuerpo).unwrap();
    assert_eq!(json["started"], true, "respuesta: {json}");
    esperar(&reg, 1).await;
    let titulos = reg.llamadas.lock().unwrap().clone();
    assert_eq!(titulos, vec!["A"]);
    assert!(driver.active_track(GUILD_U64).is_some());
}

#[tokio::test]
async fn post_play_con_fallo_de_historial_sigue_reproduciendo() {
    let reg = Arc::new(RegistradorFalso {
        falla: true,
        ..Default::default()
    });
    let (driver, _prov, ctx) = montar(reg.clone()).await;
    let respuesta = post_play_de_prueba(ctx).await;
    assert_eq!(respuesta.status(), StatusCode::OK);
    esperar(&reg, 1).await;
    assert!(driver.active_track(GUILD_U64).is_some());
}
