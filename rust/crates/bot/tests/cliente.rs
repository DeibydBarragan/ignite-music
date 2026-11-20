//! Tests de Spec S-21: Cliente twilight (un shard).

use bot::apagado::{ejecutar_apagado, ejecutar_apagado_critico, PasosApagado, SHUTDOWN_GRACE_MS};
use bot::cliente::{
    crear_presencia_arranque, despachar_interaccion, procesar_ready, HooksReady, RouterInteraccion,
    BOT_INTENTS,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use twilight_gateway::Intents;
use twilight_model::application::interaction::Interaction;
use twilight_model::gateway::payload::incoming::Ready;
use twilight_model::gateway::presence::{ActivityType, Status};
use twilight_model::id::Id;

#[test]
fn test_intents_valor_129() {
    assert_eq!(BOT_INTENTS.bits(), 129);
    assert!(BOT_INTENTS.contains(Intents::GUILDS));
    assert!(BOT_INTENTS.contains(Intents::GUILD_VOICE_STATES));
    assert!(!BOT_INTENTS.contains(Intents::MESSAGE_CONTENT));
    assert!(!BOT_INTENTS.contains(Intents::GUILD_MEMBERS));
}

#[test]
fn test_presencia_tipo_2_y_texto_status() {
    let payload = crear_presencia_arranque("/play");
    assert_eq!(payload.activities.len(), 1);
    let act = &payload.activities[0];
    assert_eq!(act.name, "/play");
    assert_eq!(act.kind, ActivityType::Listening);
    assert_eq!(u8::from(act.kind), 2);
    assert_eq!(payload.status, Status::Online);
}

struct MockHooks {
    pub al_conectar: AtomicUsize,
    pub cargar_comandos: AtomicUsize,
    pub restaurar_sesiones: AtomicUsize,
    pub iniciar_servicios: AtomicUsize,
}

#[async_trait::async_trait]
impl HooksReady for MockHooks {
    async fn al_conectar(&self, _tag: &str, _n: usize) {
        self.al_conectar.fetch_add(1, Ordering::SeqCst);
    }
    async fn cargar_comandos(&self) {
        self.cargar_comandos.fetch_add(1, Ordering::SeqCst);
    }
    async fn restaurar_sesiones(&self) {
        self.restaurar_sesiones.fetch_add(1, Ordering::SeqCst);
    }
    async fn iniciar_servicios(&self) {
        self.iniciar_servicios.fetch_add(1, Ordering::SeqCst);
    }
}

fn crear_ready_mock() -> Ready {
    let json = serde_json::json!({
        "v": 10,
        "user": { "id": "12345", "username": "Ignite", "discriminator": "0001", "avatar": null, "bot": true, "mfa_enabled": false },
        "guilds": [{ "id": "999", "unavailable": true }],
        "session_id": "test_sid", "resume_gateway_url": "wss://gateway.discord.gg", "shard": [0, 1],
        "application": { "id": "12345", "flags": 0 }
    });
    serde_json::from_value(json).expect("fallo al deserializar Ready mock")
}

#[tokio::test]
async fn test_segundo_ready_no_repite_inicializacion() {
    let hooks = MockHooks {
        al_conectar: AtomicUsize::new(0),
        cargar_comandos: AtomicUsize::new(0),
        restaurar_sesiones: AtomicUsize::new(0),
        iniciar_servicios: AtomicUsize::new(0),
    };
    let ready_hecho = AtomicBool::new(false);
    let contador_ready = AtomicUsize::new(0);
    let ready = crear_ready_mock();

    assert!(procesar_ready(&ready_hecho, &contador_ready, &ready, &hooks).await);
    assert_eq!(hooks.al_conectar.load(Ordering::SeqCst), 1);
    assert_eq!(hooks.cargar_comandos.load(Ordering::SeqCst), 1);
    assert_eq!(hooks.restaurar_sesiones.load(Ordering::SeqCst), 1);
    assert_eq!(hooks.iniciar_servicios.load(Ordering::SeqCst), 1);
    assert_eq!(contador_ready.load(Ordering::SeqCst), 1);

    assert!(!procesar_ready(&ready_hecho, &contador_ready, &ready, &hooks).await);
    assert_eq!(hooks.al_conectar.load(Ordering::SeqCst), 1);
    assert_eq!(hooks.cargar_comandos.load(Ordering::SeqCst), 1);
    assert_eq!(hooks.restaurar_sesiones.load(Ordering::SeqCst), 1);
    assert_eq!(hooks.iniciar_servicios.load(Ordering::SeqCst), 1);
    assert_eq!(contador_ready.load(Ordering::SeqCst), 2);
}

struct MockPasosApagado {
    pub secuencia: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait::async_trait]
impl PasosApagado for MockPasosApagado {
    async fn dejar_despachar_eventos(&self) -> Result<(), String> {
        self.secuencia.lock().unwrap().push("dejar_eventos");
        Ok(())
    }
    async fn cerrar_api_y_tunel(&self) -> Result<(), String> {
        self.secuencia.lock().unwrap().push("cerrar_api_y_tunel");
        Ok(())
    }
    async fn apagar_reproductor(&self) -> Result<(), String> {
        self.secuencia.lock().unwrap().push("apagar_reproductor");
        Ok(())
    }
    async fn volcar_csv_voz(&self) -> Result<(), String> {
        self.secuencia.lock().unwrap().push("volcar_csv_voz");
        Ok(())
    }
    async fn cerrar_gateway(&self) -> Result<(), String> {
        self.secuencia.lock().unwrap().push("cerrar_gateway");
        Ok(())
    }
}

#[tokio::test]
async fn test_apagado_orden_de_pasos() {
    let secuencia = Arc::new(Mutex::new(Vec::new()));
    let mock = MockPasosApagado {
        secuencia: secuencia.clone(),
    };

    assert_eq!(ejecutar_apagado(&mock, Duration::from_millis(500)).await, 0);
    assert_eq!(
        secuencia.lock().unwrap().clone(),
        vec![
            "dejar_eventos",
            "cerrar_api_y_tunel",
            "apagar_reproductor",
            "volcar_csv_voz",
            "cerrar_gateway"
        ]
    );
    assert_eq!(
        ejecutar_apagado_critico(&mock, Duration::from_millis(500)).await,
        1
    );
}

struct MockPasosLento;

#[async_trait::async_trait]
impl PasosApagado for MockPasosLento {
    async fn dejar_despachar_eventos(&self) -> Result<(), String> {
        Ok(())
    }
    async fn cerrar_api_y_tunel(&self) -> Result<(), String> {
        tokio::time::sleep(Duration::from_millis(80)).await;
        Ok(())
    }
    async fn apagar_reproductor(&self) -> Result<(), String> {
        Ok(())
    }
    async fn volcar_csv_voz(&self) -> Result<(), String> {
        Ok(())
    }
    async fn cerrar_gateway(&self) -> Result<(), String> {
        Ok(())
    }
}

#[tokio::test]
async fn test_apagado_tiempo_limite_superado() {
    let mock = MockPasosLento;
    let codigo = ejecutar_apagado(&mock, Duration::from_millis(15)).await;
    assert_eq!(codigo, 0);
}

struct MockRouter {
    pub llamadas: AtomicUsize,
}

#[async_trait::async_trait]
impl RouterInteraccion for MockRouter {
    async fn despachar(&self, _interaction: Interaction) {
        self.llamadas.fetch_add(1, Ordering::SeqCst);
    }
}

fn crear_interaccion(guild_id: Option<Id<twilight_model::id::marker::GuildMarker>>) -> Interaction {
    let json = serde_json::json!({
        "id": "9999", "application_id": "12345", "type": 2, "token": "tok", "version": 1,
        "guild_id": guild_id.map(|g| g.to_string()),
        "authorizing_integration_owners": {},
        "data": { "id": "1111", "name": "play", "type": 1 }
    });
    serde_json::from_value(json).expect("fallo al deserializar Interaction mock")
}

#[tokio::test]
async fn test_interaccion_sin_guild_id_ignorada() {
    let router = MockRouter {
        llamadas: AtomicUsize::new(0),
    };
    assert!(!despachar_interaccion(crear_interaccion(None), &router).await);
    assert_eq!(router.llamadas.load(Ordering::SeqCst), 0);

    assert!(despachar_interaccion(crear_interaccion(Some(Id::new(777))), &router).await);
    assert_eq!(router.llamadas.load(Ordering::SeqCst), 1);
}

#[test]
fn test_paridad_con_node_intents_y_presencia() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../src/bot.js");
    let bot_js = std::fs::read_to_string(&root).expect("no se pudo leer src/bot.js");
    assert!(bot_js.contains("GatewayIntentBits.Guilds"));
    assert!(bot_js.contains("GatewayIntentBits.GuildVoiceStates"));
    assert!(bot_js.contains("ActivityType.Listening"));
    assert_eq!(BOT_INTENTS.bits(), 129);
    assert_eq!(u8::from(ActivityType::Listening), 2);
    assert_eq!(SHUTDOWN_GRACE_MS, 10_000);
}
