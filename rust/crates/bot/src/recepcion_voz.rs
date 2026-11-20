use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use voice::{ReceiverConfig, SpeakerFilter, VoiceFragment, VoiceReceiverHandler, VoiceSegmenter};

pub fn preparar_recepcion(
    driver: player::SongbirdDriver,
    config: &nucleo::config_model::VoiceConfig,
    bot_id: u64,
    http: Arc<twilight_http::Client>,
) -> (
    player::SongbirdDriver,
    Arc<SpeakerFilter>,
    mpsc::Receiver<VoiceFragment>,
) {
    let filtro = Arc::new(SpeakerFilter::new(
        voice::SpeakerFilterConfig {
            bot_id: Some(bot_id),
            sound_bot_id: config.sound_bot_id.parse().ok(),
            allow_bot_input: config.allow_bot_input,
        },
        Some(http),
    ));
    let filtro_llamada = filtro.clone();
    let config = ReceiverConfig {
        silence_duration_ms: config.silence_duration_ms.clamp(400, 3000) as u32,
        ..ReceiverConfig::default()
    };
    let (tx, rx) = mpsc::channel(32);
    let entregar = Arc::new(move |fragmento: VoiceFragment| {
        if let Err(mpsc::error::TrySendError::Full(_)) = tx.try_send(fragmento) {
            tracing::warn!("Cola de fragmentos de voz llena");
        }
    });
    let driver = driver.con_receptor(move |guild, llamada| {
        configurar_receptor(
            guild,
            llamada,
            config.clone(),
            filtro_llamada.clone(),
            entregar.clone(),
        );
    });
    (driver, filtro, rx)
}

pub fn configurar_receptor(
    guild: u64,
    llamada: &mut songbird::Call,
    config: ReceiverConfig,
    filtro: Arc<SpeakerFilter>,
    entregar: Arc<dyn Fn(VoiceFragment) + Send + Sync>,
) {
    let segmentador = Arc::new(Mutex::new(VoiceSegmenter::new(guild, config, filtro)));
    VoiceReceiverHandler::new(segmentador, entregar).registrar_en(llamada);
}
