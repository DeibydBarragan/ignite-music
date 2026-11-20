//! Tests de integración para operaciones sobre la cola de reproducción (Spec S-27).

use bot::api::contexto::EstadoApiContexto;
use bot::api::crear_router_con_contexto;
use medios::Cancion;
use nucleo::Config;
use player::cola::ElementoCola;
use player::GestorColas;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

const SECRET: &str = "test-secret-0123456789abcdef0123456789abcdef";
const GUILD: &str = "123456789012345678";
const USER: &str = "987654321098765432";

async fn test_context_with_songs(n: usize) -> (EstadoApiContexto, Arc<GestorColas>) {
    let mut cfg = Config::from_map(
        std::path::PathBuf::from("."),
        &std::collections::HashMap::new(),
    );
    cfg.api.secret = SECRET.to_string();
    cfg.api.enabled = true;
    cfg.data_dir = std::env::temp_dir().join(format!("ignite_cola_test_{}", fastrand::u64(..)));
    let config = Arc::new(cfg);
    let gestor = Arc::new(GestorColas::nuevo());

    let cola = gestor.obtener_cola(GUILD).await;
    let mut guard = cola.lock().await;
    for i in 1..=n {
        let cancion = Cancion {
            fuente: "youtube".to_string(),
            id: format!("song-{i}"),
            nombre: format!("Song {i}"),
            artista: format!("Artist {i}"),
            url: format!("https://www.youtube.com/watch?v=song-{i}"),
            stream_url: None,
            es_directo: false,
            duracion: 100 + i as u64,
            miniatura: None,
            autor_url: None,
            edad_restringida: false,
            es_letra: false,
        };
        let mut elem = ElementoCola::nuevo(cancion, Some(USER.to_string()));
        elem.track_id = format!("track_id_test_{i}");
        guard.canciones.push(elem);
    }
    drop(guard);

    (EstadoApiContexto::nuevo(config, gestor.clone()), gestor)
}

async fn spawn_api(ctx: EstadoApiContexto) -> (String, tokio::task::JoinHandle<()>) {
    let app = crear_router_con_contexto(Instant::now(), ctx);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });
    (format!("http://127.0.0.1:{port}"), handle)
}

#[tokio::test]
async fn test_queue_remove_by_index_and_track_id() {
    let (ctx, _) = test_context_with_songs(4).await;
    let (base, _h) = spawn_api(ctx).await;
    let client = Client::new();

    // 1. Remove index 2 (Song 3)
    let res1 = client
        .delete(format!("{base}/api/v1/guilds/{GUILD}/queue/2"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .send()
        .await
        .unwrap();
    assert_eq!(res1.status(), StatusCode::OK);
    let j1: Value = res1.json().await.unwrap();
    let expected_remove_index: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/queue-remove-index.json"
    ))
    .unwrap();
    assert_eq!(
        json!({"status":200,"success":j1["success"],"removedTitle":j1["removed"]["title"]}),
        expected_remove_index
    );
    assert_eq!(j1["success"], true);
    assert_eq!(j1["removed"]["title"], "Song 3");

    // 2. Remove by trackId: track_id_test_2 (Song 2)
    let res2 = client
        .delete(format!(
            "{base}/api/v1/guilds/{GUILD}/queue/items/track_id_test_2"
        ))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .send()
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::OK);
    let j2: Value = res2.json().await.unwrap();
    let expected_remove_track: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/queue-remove-track.json"
    ))
    .unwrap();
    assert_eq!(
        json!({"status":200,"success":j2["success"],"removedTitle":j2["removed"]["title"]}),
        expected_remove_track
    );
    assert_eq!(j2["success"], true);
    assert_eq!(j2["removed"]["title"], "Song 2");

    // 3. Track not found
    let res3 = client
        .delete(format!(
            "{base}/api/v1/guilds/{GUILD}/queue/items/track_id_nonexistent"
        ))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .send()
        .await
        .unwrap();
    assert_eq!(res3.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        res3.json::<Value>().await.unwrap()["code"],
        "TRACK_NOT_FOUND"
    );
}

#[tokio::test]
async fn test_queue_move_order_and_jump() {
    let (ctx, gestor) = test_context_with_songs(5).await;
    let (base, _h) = spawn_api(ctx).await;
    let client = Client::new();

    // 1. Move from 3 to 1
    let r_mv = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/queue/move"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({"from": 3, "to": 1}))
        .send()
        .await
        .unwrap();
    assert_eq!(r_mv.status(), StatusCode::OK);
    assert_eq!(r_mv.json::<Value>().await.unwrap()["success"], true);

    // 2. Order track_id_test_4 to 1
    let r_ord = client
        .patch(format!("{base}/api/v1/guilds/{GUILD}/queue/order"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({"trackId": "track_id_test_4", "to": 1}))
        .send()
        .await
        .unwrap();
    assert_eq!(r_ord.status(), StatusCode::OK);
    let ordered: Value = r_ord.json().await.unwrap();
    let expected_order: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/queue-order.json"
    ))
    .unwrap();
    assert_eq!(
        json!({"status":200,"success":ordered["success"]}),
        expected_order
    );
    let cola = gestor.obtener_cola(GUILD).await;
    let ids: Vec<String> = cola
        .lock()
        .await
        .canciones
        .iter()
        .map(|item| item.cancion.id.clone())
        .collect();
    assert_eq!(ids, ["song-1", "song-4", "song-2", "song-3", "song-5"]);

    // 3. Jump to position 2
    let r_jmp = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/queue/jump"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({"position": 2}))
        .send()
        .await
        .unwrap();
    assert_eq!(r_jmp.status(), StatusCode::OK);
    assert_eq!(r_jmp.json::<Value>().await.unwrap()["success"], true);
}

#[tokio::test]
async fn test_queue_clear_and_undo() {
    let (ctx, gestor) = test_context_with_songs(4).await;
    let (base, _h) = spawn_api(ctx).await;
    let client = Client::new();

    // Clear queue (keeps current song at 0, clears remaining 3)
    let r_clr = client
        .delete(format!("{base}/api/v1/guilds/{GUILD}/queue"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .send()
        .await
        .unwrap();
    assert_eq!(r_clr.status(), StatusCode::OK);
    let j_clr: Value = r_clr.json().await.unwrap();
    let contrato: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/queue-clear.json"
    ))
    .unwrap();
    assert_eq!(
        j_clr, contrato,
        "Rust queue clear response must match the shared cross-runtime contract"
    );
    assert_eq!(j_clr["success"], true);
    assert_eq!(j_clr["cleared"], 3);

    // Undo within TTL
    let r_undo = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/queue/undo"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .send()
        .await
        .unwrap();
    assert_eq!(r_undo.status(), StatusCode::OK);
    let j_undo: Value = r_undo.json().await.unwrap();
    let expected_undo: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/queue-undo.json"
    ))
    .unwrap();
    assert_eq!(
        json!({"status":200,"success":j_undo["success"],"restored":j_undo["restored"]}),
        expected_undo
    );
    assert_eq!(j_undo["success"], true);
    assert_eq!(j_undo["restored"], 3);

    // Undo again (now empty cache) responds 400 UNDO_EMPTY
    let r_empty = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/queue/undo"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .send()
        .await
        .unwrap();
    assert_eq!(r_empty.status(), StatusCode::BAD_REQUEST);
    assert_eq!(r_empty.json::<Value>().await.unwrap()["code"], "UNDO_EMPTY");

    // Clear and expire undo cache manually
    gestor.guardar_deshacer(GUILD, vec![], 0).await;
    let r_exp = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/queue/undo"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .send()
        .await
        .unwrap();
    assert_eq!(r_exp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_queue_missing_matches_shared_problem_contract() {
    let (ctx, _) = test_context_with_songs(0).await;
    let (base, _h) = spawn_api(ctx).await;
    let response = Client::new()
        .delete(format!("{base}/api/v1/guilds/{GUILD}/queue"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    let problem: Value = response.json().await.unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/queue-missing.json"
    ))
    .unwrap();
    assert_eq!(json!({"status": status, "code": problem["code"]}), expected);
}
