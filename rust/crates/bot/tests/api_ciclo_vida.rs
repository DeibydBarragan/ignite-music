//! Test del ciclo de vida del servidor API y apagado limpio (Spec S-26).

use bot::api::iniciar_servidor_api;
use std::sync::Arc;
use std::time::Instant;

const TEST_SECRET: &str = "test_secret_with_32_characters_12";

#[tokio::test]
async fn test_ciclo_de_vida_servidor_y_apagado() {
    let mut config = nucleo::Config::from_map(
        std::path::PathBuf::from("."),
        &std::collections::HashMap::new(),
    );
    config.api.enabled = true;
    config.api.port = 0;
    config.api.secret = TEST_SECRET.to_string();

    let server_handle = iniciar_servidor_api(Arc::new(config), Instant::now())
        .await
        .unwrap();
    let server = server_handle.unwrap();
    let mut stream = reqwest::Client::new()
        .get(format!(
            "http://127.0.0.1:{}/api/v1/guilds/123456789012345678/events",
            server.port
        ))
        .bearer_auth(TEST_SECRET)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert!(stream.chunk().await.unwrap().is_some());
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        let apagar = tokio::spawn(server.apagar());
        while stream.chunk().await.unwrap().is_some() {}
        apagar.await.unwrap();
    })
    .await
    .expect("el apagado debe cerrar la suscripción activa");
}
