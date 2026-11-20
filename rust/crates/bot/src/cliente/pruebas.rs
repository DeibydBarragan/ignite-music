use super::*;
use tokio::sync::mpsc;
use twilight_model::gateway::payload::incoming::{VoiceServerUpdate, VoiceStateUpdate};
use twilight_model::id::Id;

struct Hooks;

#[async_trait::async_trait]
impl HooksReady for Hooks {
    async fn al_conectar(&self, _: &str, _: usize) {}
    async fn cargar_comandos(&self) {}
    async fn restaurar_sesiones(&self) {}
    async fn iniciar_servicios(&self) {}
}

struct RouterPrueba {
    driver: Arc<player::SongbirdDriver>,
    mensajes: mpsc::UnboundedSender<&'static str>,
}

#[async_trait::async_trait]
impl RouterInteraccion for RouterPrueba {
    async fn despachar(&self, interaction: Interaction) {
        if interaction.id.get() != 1 {
            self.mensajes.send("segunda interacción").unwrap();
            return;
        }
        let call = self.driver.get_or_insert(1).unwrap();
        let canal = Id::<twilight_model::id::marker::ChannelMarker>::new(3);
        let espera = call.lock().await.join_gateway(canal).await.unwrap();
        self.mensajes.send("esperando voz").unwrap();
        let conexion = espera.await.unwrap();
        assert_eq!(conexion.session_id, "sesion-de-prueba");
        self.mensajes.send("conexión preparada").unwrap();
    }
}

fn interaccion(id: u64) -> Event {
    use twilight_model::gateway::payload::incoming::InteractionCreate;
    let payload = serde_json::json!({
        "id": id.to_string(), "application_id": "2", "type": 2, "token": "test",
        "guild_id": "1", "authorizing_integration_owners": {}, "version": 1,
        "data": {"id": "6", "name": "play", "type": 1, "options": []}
    });
    Event::InteractionCreate(Box::new(InteractionCreate(
        serde_json::from_value(payload).unwrap(),
    )))
}

#[tokio::test]
async fn el_gateway_entrega_ambos_eventos_mientras_el_comando_espera() {
    let (_shard, sender) = crear_shard("token-de-prueba", "prueba");
    let songbird = player::SongbirdDriver::create_songbird(Id::new(2), [(0, sender)].into());
    let driver = Arc::new(player::SongbirdDriver::new(songbird));
    let (tx, mut rx) = mpsc::unbounded_channel();
    let despachador = DespachadorEventos {
        cache: Arc::new(crear_cache()),
        ready_hecho: Default::default(),
        contador_ready: Default::default(),
        hooks_ready: Arc::new(Hooks),
        router: Arc::new(RouterPrueba {
            driver: driver.clone(),
            mensajes: tx,
        }),
        driver: Some(driver),
        gestor_presencia: None,
        acciones_voz: None,
        filtro_voz: None,
        latencia: Default::default(),
    };
    let mut trabajos = TrabajosGateway::nuevo(None, None);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        despachador
            .procesar_evento(interaccion(1), &mut trabajos)
            .await
            .unwrap();
        assert_eq!(rx.recv().await, Some("esperando voz"));
        let estado: VoiceStateUpdate = serde_json::from_value(serde_json::json!({
            "guild_id": "1", "user_id": "2", "channel_id": "3",
            "session_id": "sesion-de-prueba", "deaf": false, "mute": false,
            "self_deaf": false, "self_mute": false, "self_video": false, "suppress": false
        }))
        .unwrap();
        despachador
            .procesar_evento(Event::VoiceStateUpdate(Box::new(estado)), &mut trabajos)
            .await
            .unwrap();
        despachador
            .procesar_evento(interaccion(4), &mut trabajos)
            .await
            .unwrap();
        assert_eq!(rx.recv().await, Some("segunda interacción"));
        let servidor = VoiceServerUpdate {
            endpoint: Some("prueba.invalid:443".into()),
            guild_id: Id::new(1),
            token: "test".into(),
        };
        despachador
            .procesar_evento(Event::VoiceServerUpdate(servidor), &mut trabajos)
            .await
            .unwrap();
        assert_eq!(rx.recv().await, Some("conexión preparada"));
    })
    .await
    .expect("El gateway quedó esperando el comando o faltó un evento de voz");
}
