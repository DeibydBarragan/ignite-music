use player::driver::{DriverError, SongbirdDriver};
use player::{Cola, ElementoCola};
use std::collections::HashMap;
use std::path::Path;
use twilight_model::id::Id;

#[test]
fn test_driver_error_display() {
    let err = DriverError::Input("missing file".into());
    assert_eq!(err.to_string(), "File input error: missing file");

    let err2 = DriverError::NotInChannel(123);
    assert_eq!(
        err2.to_string(),
        "Not connected to a voice channel in guild 123"
    );

    let err3 = DriverError::NoTrack(456);
    assert_eq!(err3.to_string(), "No active track in guild 456");
}

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
async fn filtro_centralizado_reinstala_y_no_reinicia_si_se_repite() {
    let driver = driver_prueba();
    driver.get_or_insert(10).unwrap();
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
    let primera = driver.active_track(10).unwrap();
    cola.posicion_segundos = 37;

    let res = player::reproduccion::fijar_filtro_con_driver(
        Some(&driver),
        &mut cola,
        "bassboost",
        "yt-dlp",
        Vec::new(),
        &settings,
    )
    .await
    .unwrap();
    assert!(res.ok);
    assert_eq!(cola.posicion_segundos, 37);
    let filtrada = driver.active_track(10).unwrap();
    assert_ne!(primera.uuid(), filtrada.uuid());

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
    assert_eq!(driver.active_track(10).unwrap().uuid(), filtrada.uuid());
}

#[tokio::test]
async fn seek_centralizado_actualiza_posicion_y_reinstala_la_pista() {
    let driver = driver_prueba();
    driver.get_or_insert(10).unwrap();
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
    let original = driver.active_track(10).unwrap();

    let resultado = player::reproduccion::buscar_con_driver(
        Some(&driver),
        &mut cola,
        45,
        "yt-dlp",
        Vec::new(),
        &settings,
    )
    .await
    .unwrap();
    assert!(resultado.ok);
    assert_eq!(cola.posicion_segundos, 45);
    assert_ne!(original.uuid(), driver.active_track(10).unwrap().uuid());
}

#[tokio::test]
async fn pausa_centralizada_sincroniza_cola_y_songbird_y_es_idempotente() {
    let driver = driver_prueba();
    driver.get_or_insert(10).unwrap();
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
    let pista = driver.active_track(10).unwrap();

    let pausada = player::reproduccion::fijar_pausa_con_driver(Some(&driver), &mut cola, true)
        .await
        .unwrap();
    assert!(pausada.ok);
    assert!(cola.pausada);
    assert_eq!(driver.active_track(10).unwrap().uuid(), pista.uuid());

    let repetida = player::reproduccion::fijar_pausa_con_driver(Some(&driver), &mut cola, true)
        .await
        .unwrap();
    assert!(!repetida.ok);
    assert!(cola.pausada);

    let reanudada = player::reproduccion::alternar_pausa_con_driver(Some(&driver), &mut cola)
        .await
        .unwrap();
    assert!(reanudada.ok);
    assert!(!cola.pausada);
}

#[tokio::test]
async fn pausa_centralizada_revierte_cola_si_no_hay_handle_en_songbird() {
    let driver = driver_prueba();
    let mut cola = cola_prueba("10");

    let resultado =
        player::reproduccion::fijar_pausa_con_driver(Some(&driver), &mut cola, true).await;

    assert!(matches!(resultado, Err(DriverError::NoTrack(10))));
    assert!(!cola.pausada);

    cola.guild_id = "guild-invalido".into();
    let resultado =
        player::reproduccion::fijar_pausa_con_driver(Some(&driver), &mut cola, true).await;
    assert!(matches!(resultado, Err(DriverError::Input(_))));
    assert!(!cola.pausada);
}

#[tokio::test]
async fn saltar_anterior_stop_y_volumen_comparten_operaciones_de_driver() {
    let driver = driver_prueba();
    driver.get_or_insert(10).unwrap();
    let settings = medios::YtDlpSettings::default();
    let mut cola = cola_prueba("10");
    let mut siguiente = cola.canciones[0].clone();
    siguiente.cancion.id = "track-2".into();
    siguiente.cancion.nombre = "Second track".into();
    cola.canciones.push(siguiente);
    player::reproduccion::iniciar_pista_con_settings(
        &driver,
        &mut cola,
        "yt-dlp",
        Vec::new(),
        &settings,
    )
    .await
    .unwrap();

    let saltada = player::reproduccion::cambiar_pista_con_driver(Some(&driver), &mut cola, false)
        .await
        .unwrap();
    assert!(saltada.ok);
    assert_eq!(cola.canciones[0].cancion.nombre, "Second track");
    assert!(driver.active_track(10).is_none());

    player::reproduccion::cambiar_pista_con_driver(Some(&driver), &mut cola, true)
        .await
        .unwrap();
    assert_eq!(cola.canciones[0].cancion.nombre, "Test track");

    let error_volumen =
        player::reproduccion::fijar_volumen_con_driver(Some(&driver), &mut cola, 50, 100).await;
    assert!(matches!(error_volumen, Err(DriverError::NoTrack(10))));
    assert_eq!(cola.volumen, player::DEFAULT_VOLUME);

    let detenida = player::reproduccion::parar_con_driver(Some(&driver), &mut cola)
        .await
        .unwrap();
    assert!(detenida.ok);
    assert!(cola.canciones.is_empty());
    assert!(cola.detenida);
}

#[tokio::test]
async fn test_play_file_nonexistent_returns_error() {
    let songbird = SongbirdDriver::create_songbird(Id::new(1), HashMap::new());
    let driver = SongbirdDriver::new(songbird);

    let res = driver.play_file(10, Path::new("nonexistent.wav")).await;
    assert!(matches!(res, Err(DriverError::Input(_))));
}

#[tokio::test]
async fn test_pause_resume_no_track() {
    let songbird = SongbirdDriver::create_songbird(Id::new(1), HashMap::new());
    let driver = SongbirdDriver::new(songbird);

    assert!(matches!(
        driver.pause(10).await,
        Err(DriverError::NoTrack(10))
    ));
    assert!(matches!(
        driver.resume(10).await,
        Err(DriverError::NoTrack(10))
    ));
    assert!(driver.stop(10).await.is_ok());
}
#[tokio::test]
async fn un_join_fallido_se_puede_reintentar_sin_duplicar_el_receptor() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let registros = Arc::new(AtomicUsize::new(0));
    let contador = registros.clone();
    let shard = twilight_gateway::Shard::new(
        twilight_gateway::ShardId::ONE,
        "token-de-prueba".into(),
        twilight_gateway::Intents::GUILD_VOICE_STATES,
    );
    let songbird = SongbirdDriver::create_songbird(Id::new(1), [(0, shard.sender())].into());
    drop(shard);
    let driver = SongbirdDriver::new(songbird).con_receptor(move |_, _| {
        contador.fetch_add(1, Ordering::Relaxed);
    });
    assert!(driver.join(1, 2).await.is_err());
    assert!(driver.join(1, 2).await.is_err());
    assert_eq!(registros.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn un_fin_tardio_o_duplicado_no_consume_otro_handle() {
    let shard = twilight_gateway::Shard::new(
        twilight_gateway::ShardId::ONE,
        "token-de-prueba".into(),
        twilight_gateway::Intents::GUILD_VOICE_STATES,
    );
    let songbird = SongbirdDriver::create_songbird(Id::new(1), [(0, shard.sender())].into());
    let driver = SongbirdDriver::new(songbird);
    driver.get_or_insert(10).unwrap();
    let primera = driver
        .play_ytdl(10, "example", "yt-dlp", vec![], 60, true)
        .await
        .unwrap();
    let segunda = driver
        .play_ytdl(10, "example", "yt-dlp", vec![], 0, true)
        .await
        .unwrap();
    assert!(!driver.aceptar_fin(10, primera.uuid().as_u128()));
    assert_eq!(driver.active_track(10).unwrap().uuid(), segunda.uuid());
    assert!(driver.aceptar_fin(10, segunda.uuid().as_u128()));
    assert!(!driver.aceptar_fin(10, segunda.uuid().as_u128()));
}
