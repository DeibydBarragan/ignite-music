//! Live smoke test. Uses explicit test credentials and measured speech durations.
//! Reads `.env.test` from the repository root; process environment overrides file values.
use bot::cliente::{crear_shard, BOT_INTENTS};
use bot::recepcion_voz::configurar_receptor;
use player::SongbirdDriver;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::mpsc;
use twilight_gateway::{EventTypeFlags, StreamExt};
use twilight_http::Client;
use voice::{ReceiverConfig, SpeakerFilter, SpeakerFilterConfig};

fn cargar_env_test() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../.env.test");
    if path.is_file() {
        dotenvy::from_path(path).expect("No se pudo cargar el .env.test de prueba");
    }
}

fn requerido(nombre: &str) -> String {
    let valor = std::env::var(nombre).unwrap_or_else(|_| panic!("Falta {nombre}"));
    assert!(!valor.trim().is_empty(), "Falta {nombre}");
    valor
}

#[tokio::test]
#[ignore = "requiere aplicación, canal DAVE y locutor humano de prueba"]
async fn recibe_pcm_humano_mientras_suena_musica_y_despues_de_reconectar() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    cargar_env_test();
    let token = requerido("IGNITE_VOICE_TEST_TOKEN");
    let guild = requerido("IGNITE_VOICE_TEST_GUILD_ID").parse().unwrap();
    let canal = requerido("IGNITE_VOICE_TEST_CHANNEL_ID").parse().unwrap();
    let usuario = requerido("IGNITE_VOICE_TEST_SPEAKER_ID")
        .parse::<u64>()
        .unwrap();
    let duraciones: Vec<u32> = requerido("IGNITE_VOICE_TEST_EXPECTED_MS")
        .split(',')
        .map(|ms| ms.trim().parse().expect("Duración de referencia en ms"))
        .collect();
    assert_eq!(duraciones.len(), 10, "Se requieren diez duraciones");
    let wav =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../test/fixtures/ignite-sample.wav");
    assert!(wav.is_file(), "Falta el WAV de prueba");
    let http = Arc::new(Client::new(token.clone()));
    let bot = http.current_user().await.unwrap().model().await.unwrap();
    assert_ne!(bot.id.get(), usuario);
    let (mut shard, sender) = crear_shard(&token, "Prueba de recepción");
    assert!(BOT_INTENTS.contains(twilight_gateway::Intents::GUILD_VOICE_STATES));
    let songbird = SongbirdDriver::create_songbird(bot.id, [(0, sender)].into());
    let filtro = Arc::new(SpeakerFilter::new(
        SpeakerFilterConfig {
            bot_id: Some(bot.id.get()),
            ..Default::default()
        },
        Some(http),
    ));
    let (tx, mut rx) = mpsc::channel(32);
    let driver = SongbirdDriver::new(songbird.clone()).con_receptor(move |guild, llamada| {
        let tx = tx.clone();
        configurar_receptor(
            guild,
            llamada,
            ReceiverConfig::default(),
            filtro.clone(),
            Arc::new(move |fragmento| {
                let _ = tx.try_send(fragmento);
            }),
        );
    });
    let (listo, conectado) = tokio::sync::oneshot::channel();
    let gateway = tokio::spawn(async move {
        let mut listo = Some(listo);
        while let Some(evento) = shard.next_event(EventTypeFlags::all()).await {
            let Ok(evento) = evento else {
                continue;
            };
            if matches!(evento, twilight_gateway::Event::Ready(_)) {
                if let Some(listo) = listo.take() {
                    let _ = listo.send(());
                }
            }
            songbird.process(&evento).await;
        }
    });
    tokio::time::timeout(Duration::from_secs(25), conectado)
        .await
        .expect("Discord no envió READY")
        .expect("El gateway terminó antes de READY");

    for intento in 1..=2 {
        driver
            .join(guild, canal)
            .await
            .expect("Conectar al canal DAVE");
        let pista = driver.play_file(guild, &wav).await.expect("Cargar WAV");
        pista
            .enable_loop()
            .expect("Repetir WAV mientras habla el locutor");
        tokio::time::timeout(Duration::from_secs(5), async {
            while pista
                .get_info()
                .await
                .expect("Leer estado de pista")
                .position
                .is_zero()
            {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("La pista no empezó a avanzar");
        tracing::info!(
            intento,
            "Pronuncia las diez frases con una pausa entre cada una"
        );
        let resultado = tokio::time::timeout(Duration::from_secs(120), async {
            for esperada in &duraciones {
                let fragmento = rx.recv().await.expect("Canal de recepción cerrado");
                assert_eq!(fragmento.guild_id, guild);
                assert_eq!(fragmento.user_id, usuario);
                assert!(fragmento.pcm48k.iter().any(|muestra| *muestra != 0));
                tracing::info!(observada = fragmento.duration_ms, esperada, "PCM de voz");
                assert!(fragmento.duration_ms.abs_diff(*esperada) <= 100);
            }
            assert!(
                tokio::time::timeout(Duration::from_secs(2), rx.recv())
                    .await
                    .is_err(),
                "Se recibió PCM adicional sin una nueva frase"
            );
        })
        .await;
        driver.stop(guild).await.expect("Detener WAV");
        driver
            .leave(guild)
            .await
            .expect("Salir del canal de prueba");
        resultado.expect("No se recibieron diez frases humanas");
    }
    gateway.abort();
}
