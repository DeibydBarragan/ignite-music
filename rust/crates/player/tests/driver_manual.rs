//! Manual integration test for Songbird voice driver (Spec S-16).
//!
//! Marked with `#[ignore]` so it is skipped during CI and automated runs.
//! Can be executed manually with a valid Discord token and voice channel:
//! `cargo test -p player --test driver_manual -- --ignored`
//! Reads `.env.test` from the repository root; process environment overrides file values.

use player::driver::SongbirdDriver;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use tokio::time::sleep;
use twilight_gateway::{EventTypeFlags, Intents, Shard, ShardId, StreamExt};
use twilight_http::Client as HttpClient;

fn cargar_env_test() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../.env.test");
    if path.is_file() {
        dotenvy::from_path(path).expect("No se pudo cargar el .env.test de prueba");
    }
}

#[tokio::test]
#[ignore]
async fn test_manual_voice_driver_lifecycle() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    cargar_env_test();
    let token = std::env::var("IGNITE_VOICE_TEST_TOKEN").expect("Explicit Discord test token");
    assert!(!token.trim().is_empty(), "Test token is empty");
    let guild_id: u64 = std::env::var("IGNITE_VOICE_TEST_GUILD_ID")
        .expect("Explicit test guild")
        .parse()
        .expect("Valid GUILD_ID");
    let channel_id: u64 = std::env::var("IGNITE_VOICE_TEST_CHANNEL_ID")
        .expect("Explicit test channel")
        .parse()
        .expect("Valid channel ID");
    let wav =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../test/fixtures/ignite-sample.wav");
    assert!(wav.is_file(), "Manual voice fixture is missing");

    let http = HttpClient::new(token.clone());
    let current_user = http
        .current_user()
        .await
        .expect("Current user")
        .model()
        .await
        .expect("User model");
    let bot_id = current_user.id;

    let intents = Intents::GUILDS | Intents::GUILD_VOICE_STATES;
    let mut shard = Shard::new(ShardId::ONE, token, intents);

    let mut senders = HashMap::new();
    senders.insert(shard.id().number(), shard.sender());

    let songbird = SongbirdDriver::create_songbird(bot_id, senders);
    let driver = SongbirdDriver::new(songbird.clone());

    let songbird_clone = songbird.clone();
    let gateway_task = tokio::spawn(async move {
        while let Some(Ok(event)) = shard.next_event(EventTypeFlags::all()).await {
            songbird_clone.process(&event).await;
        }
    });

    sleep(Duration::from_secs(3)).await;

    driver
        .join(guild_id, channel_id)
        .await
        .expect("Join voice channel");
    sleep(Duration::from_secs(2)).await;

    let track = driver.play_file(guild_id, wav).await.expect("Play file");
    track.set_volume(0.5).expect("Set volume");

    let inicio = track
        .get_info()
        .await
        .expect("Read initial track state")
        .position;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let posicion = track
                .get_info()
                .await
                .expect("Read playback progress")
                .position;
            if posicion >= inicio + Duration::from_secs(1) {
                tracing::info!(
                    ?posicion,
                    "Outgoing audio advanced; confirm the test WAV is audible in Discord"
                );
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Outgoing audio did not advance");

    driver.pause(guild_id).await.expect("Pause active track");
    driver.resume(guild_id).await.expect("Resume active track");
    driver.stop(guild_id).await.expect("Stop playback");

    driver.leave(guild_id).await.expect("Leave voice channel");
    sleep(Duration::from_secs(1)).await;

    gateway_task.abort();
}
