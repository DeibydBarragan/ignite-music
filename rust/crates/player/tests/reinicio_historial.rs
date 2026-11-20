use player::driver::SongbirdDriver;
use player::{Cola, ElementoCola};
use std::sync::{Arc, Mutex};
use twilight_model::id::Id;

fn cola_prueba(guild_id: &str) -> Cola {
    let mut cola = Cola::nueva(guild_id);
    let cancion = medios::Cancion {
        fuente: "youtube".into(),
        id: "track-1".into(),
        nombre: "Test track".into(),
        artista: "Artist".into(),
        url: "https://example.invalid/watch?v=track-1".into(),
        stream_url: None,
        es_directo: false,
        duracion: 180,
        miniatura: None,
        autor_url: None,
        edad_restringida: false,
        es_letra: false,
    };
    cola.canciones.push(ElementoCola::nuevo(cancion, None));
    cola
}

fn driver_prueba() -> SongbirdDriver {
    let shard = twilight_gateway::Shard::new(
        twilight_gateway::ShardId::ONE,
        "token-de-prueba".into(),
        twilight_gateway::Intents::GUILD_VOICE_STATES,
    );
    let songbird = SongbirdDriver::create_songbird(Id::new(1), [(0, shard.sender())].into());
    drop(shard);
    SongbirdDriver::new(songbird)
}

#[tokio::test]
async fn seek_y_filtro_no_emiten_arranque_para_historial() {
    let driver = driver_prueba();
    driver.get_or_insert(10).unwrap();
    let eventos: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
    let registro = eventos.clone();
    driver.fijar_oyente_pista(Arc::new(move |_, evento, _| {
        registro.lock().unwrap().push(evento);
    }));
    let settings = medios::YtDlpSettings::default();
    let mut cola = cola_prueba("10");

    player::reproduccion::iniciar_pista_con_settings(
        &driver,
        &mut cola,
        "yt-dlp",
        Vec::new(),
        &settings,
    )
    .await
    .unwrap();
    assert_eq!(*eventos.lock().unwrap(), vec!["play"]);

    player::reproduccion::fijar_filtro_con_driver(
        Some(&driver),
        &mut cola,
        "bassboost",
        "yt-dlp",
        Vec::new(),
        &settings,
    )
    .await
    .unwrap();
    player::reproduccion::buscar_con_driver(
        Some(&driver),
        &mut cola,
        10,
        "yt-dlp",
        Vec::new(),
        &settings,
    )
    .await
    .unwrap();
    assert_eq!(
        *eventos.lock().unwrap(),
        vec!["play", "reinicio", "reinicio"],
        "seek y filtro no deben emitir \"play\""
    );

    player::reproduccion::iniciar_pista_desde_con_settings(
        &driver,
        &mut cola,
        "yt-dlp",
        Vec::new(),
        Some(0),
        &settings,
    )
    .await
    .unwrap();
    assert_eq!(
        eventos.lock().unwrap().last().copied(),
        Some("play"),
        "un arranque nuevo sigue emitiendo \"play\""
    );
}
