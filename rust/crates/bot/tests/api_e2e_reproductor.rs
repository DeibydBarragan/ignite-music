//! Test de integración end-to-end para el ciclo completo del reproductor y la API (Specs S-26..S-30).

mod common;

use common::*;
use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};

#[tokio::test]
async fn test_e2e_reproduccion_y_controles() {
    let (base, _gestor_colas, hub_sse) = spawn_e2e_api().await;
    let client = Client::new();

    // 1. Verificar estado inicial sin cola
    let r1 = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/player"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(r1.status(), StatusCode::OK);
    let j1: Value = r1.json().await.unwrap();
    assert_eq!(j1["hasQueue"], false);

    // 2. Suscribirse a SSE para escuchar player.sync
    let (_sub_id, mut stream) = hub_sse.suscribir(GUILD);
    let conn_event = stream.next().await.unwrap().unwrap();
    assert!(String::from_utf8_lossy(&conn_event).contains(": connected"));

    // 3. POST /play con canción real
    let r_play = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/play"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({
            "query": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "voiceChannelId": VOICE_CH
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r_play.status(), StatusCode::OK);
    let mut j_play: Value = r_play.json().await.unwrap();
    let track_id = j_play["song"]
        .as_object_mut()
        .unwrap()
        .remove("trackId")
        .unwrap();
    assert!(track_id.as_str().is_some_and(|id| id.len() >= 8));
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/discovery-play.json"
    ))
    .unwrap();
    assert_eq!(j_play, fixture["playSuccess"]);

    // Verificar que SSE recibió player.sync
    let sync_event = stream.next().await.unwrap().unwrap();
    assert!(String::from_utf8_lossy(&sync_event).contains("event: player.sync"));

    // 4. GET /player refleja la reproducción activa
    let r_player = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/player"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(r_player.status(), StatusCode::OK);
    let j_player: Value = r_player.json().await.unwrap();
    assert_eq!(j_player["hasQueue"], true);
    assert_eq!(j_player["isPlaying"], true);
    assert_eq!(j_player["isPaused"], false);

    // 5. POST /control pause -> pausada
    let r_pause = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/control"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({"action": "pause"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r_pause.status(), StatusCode::OK);

    let r_player_paused = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/player"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    let j_p: Value = r_player_paused.json().await.unwrap();
    assert_eq!(j_p["isPaused"], true);

    // 6. POST /control volume 50%
    let r_vol = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/control"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({"action": "volume", "value": 50}))
        .send()
        .await
        .unwrap();
    assert_eq!(r_vol.status(), StatusCode::OK);

    // 7. POST /control filter bassboost
    let r_filt = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/control"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({"action": "filter", "value": "bassboost"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r_filt.status(), StatusCode::OK);

    // 8. POST /control filter inválido devuelve un resultado sin cambio
    let r_bad_filt = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/control"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({"action": "filter", "value": "invalid_filter_name"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r_bad_filt.status(), StatusCode::OK);
    assert_eq!(r_bad_filt.json::<Value>().await.unwrap()["success"], false);

    // 9. POST /control stop -> detiene y limpia
    let r_stop = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/control"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({"action": "stop"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r_stop.status(), StatusCode::OK);

    let r_player_stopped = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/player"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(
        r_player_stopped.json::<Value>().await.unwrap()["hasQueue"],
        false
    );

    // 10. Probar fallo explícito cuando Spotify no está configurado
    let r_spot = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/play"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({"query": "https://open.spotify.com/track/unconfigured-spotify"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r_spot.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        r_spot.json::<Value>().await.unwrap()["code"],
        "SPOTIFY_NOT_CONFIGURED"
    );
}

#[tokio::test]
async fn controles_comparten_respuestas_y_estados_sse_con_node() {
    let (base, _, hub) = spawn_e2e_api().await;
    let base = format!("{base}/api/v1/guilds/{GUILD}");
    let client = Client::new();
    let post = |route, body: &Value| {
        client
            .post(format!("{base}/{route}"))
            .bearer_auth(SECRET)
            .header("x-user-discord-id", USER)
            .json(body)
    };
    let obtener_estado = || async {
        client
            .get(format!("{base}/player"))
            .bearer_auth(SECRET)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    };
    post("play", &json!({"query": "test song"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let (_, mut stream) = hub.suscribir(GUILD);
    stream.next().await.unwrap().unwrap();
    for body in [
        json!({"action":"filter", "value": {}}),
        json!({"action":""}),
        json!({}),
    ] {
        let response = post("control", &body).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.json::<Value>().await.unwrap()["code"],
            "INVALID_PARAMETER"
        );
    }

    let fixture: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/player-controls.json"
    ))
    .unwrap();
    let mensajes: Value =
        serde_json::from_str(include_str!("../../../../languages/en.json")).unwrap();
    let mut esperado = obtener_estado().await;
    for caso in fixture {
        let request = &caso["request"];
        let response = post("control", request).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{request}");
        let mut respuesta_esperada = caso["response"].clone();
        if let Some(key) = respuesta_esperada
            .as_object_mut()
            .unwrap()
            .remove("messageKey")
        {
            respuesta_esperada["message"] = json!(nucleo::idioma::clean(
                nucleo::idioma::lookup(&mensajes, key.as_str().unwrap()).unwrap()
            ));
        }
        assert_eq!(
            response.json::<Value>().await.unwrap(),
            respuesta_esperada,
            "{request}"
        );
        esperado
            .as_object_mut()
            .unwrap()
            .extend(caso["state"].as_object().unwrap().clone());
        let actual = obtener_estado().await;
        for field in [
            "volume",
            "currentTime",
            "filter",
            "isPaused",
            "isPlaying",
            "repeatMode",
            "autoplay",
        ] {
            assert_eq!(actual[field], esperado[field], "{request}: {field}");
        }
        let bytes = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let event = std::str::from_utf8(&bytes).unwrap();
        assert!(event.starts_with("event: player.sync\n"));
        let snapshot: Value = serde_json::from_str(
            event
                .lines()
                .find_map(|l| l.strip_prefix("data: "))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(snapshot, actual, "{request}");
    }
    hub.cerrar();
    assert!(stream.next().await.is_none());
}
