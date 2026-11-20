//! Tests de integración para rutas REST v1 de ignite-music (Spec S-27).

use bot::api::contexto::EstadoApiContexto;
use bot::api::crear_router_con_contexto;
use bot::api::formas::CanalVozInfo;
use nucleo::Config;
use player::GestorColas;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

const SECRET: &str = "test-secret-0123456789abcdef0123456789abcdef";
const GUILD: &str = "123456789012345678";
const USER: &str = "987654321098765432";
const VOICE: &str = "111222333444555666";

struct ProveedorCanalVoz;

struct ProveedorPlayFlags(Arc<std::sync::Mutex<Vec<bool>>>);

#[async_trait::async_trait]
impl bot::api::contexto::ProveedorApi for ProveedorCanalVoz {
    fn obtener_info_canal_voz(&self, _: &str, channel_id: &str) -> Option<CanalVozInfo> {
        Some(CanalVozInfo {
            id: channel_id.into(),
            name: "General".into(),
        })
    }
}

#[async_trait::async_trait]
impl bot::api::contexto::ProveedorApi for ProveedorPlayFlags {
    async fn reproducir(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: Option<&str>,
        _: bool,
        _: bool,
        dev_mode: bool,
    ) -> Result<bot::api::formas::ResultadoReproducirApi, (u16, &'static str, String)> {
        self.0.lock().unwrap().push(dev_mode);
        Ok(bot::api::formas::ResultadoReproducirApi {
            success: true,
            started: true,
            position: 0,
            song: None,
            playlist: None,
        })
    }
}

fn test_context() -> EstadoApiContexto {
    let mut cfg = Config::from_map(
        std::path::PathBuf::from("."),
        &std::collections::HashMap::new(),
    );
    cfg.api.secret = SECRET.to_string();
    cfg.api.enabled = true;
    cfg.data_dir = std::env::temp_dir().join(format!("ignite_api_test_{}", fastrand::u64(..)));
    let config = Arc::new(cfg);
    let gestor = Arc::new(GestorColas::nuevo());
    EstadoApiContexto::nuevo(config, gestor)
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
async fn test_player_state_empty_and_legacy_api_alias() {
    let (base, _h) = spawn_api(test_context()).await;
    let client = Client::new();

    let res_v1 = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/player"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(res_v1.status(), StatusCode::OK);
    let j_v1: Value = res_v1.json().await.unwrap();
    let contrato: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/player-empty.json"
    ))
    .unwrap();
    assert_eq!(
        j_v1, contrato,
        "Rust player state must match shared cross-runtime contract fixture"
    );
    assert_eq!(j_v1["hasQueue"], false);
    assert_eq!(j_v1["volume"], 100);

    let res_leg = client
        .get(format!("{base}/api/guilds/{GUILD}/player"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(res_leg.status(), StatusCode::OK);
    let j_leg: Value = res_leg.json().await.unwrap();
    assert_eq!(j_leg["hasQueue"], false);

    let invalid = client
        .get(format!("{base}/api/v1/guilds/invalid/player"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    let invalid_status = invalid.status().as_u16();
    let problem: Value = invalid.json().await.unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/invalid-snowflake.json"
    ))
    .unwrap();
    assert_eq!(
        json!({"status": invalid_status, "code": problem["code"]}),
        expected
    );
}

#[tokio::test]
async fn player_state_matches_full_active_node_fixture() {
    let ctx = test_context().con_proveedor(Arc::new(ProveedorCanalVoz));
    let cola = ctx.gestor_colas.obtener_cola(GUILD).await;
    {
        let mut queue = cola.lock().await;
        queue.canal_voz = Some(VOICE.into());
        queue.pausada = false;
        queue.posicion_segundos = 42;
        queue.volumen = 65;
        queue.modo_repeticion = player::ModoRepeticion::Cancion;
        queue.autoplay = true;
        queue.filtros = vec!["bassboost".into()];
        let mut current = player::ElementoCola::nuevo(
            medios::Cancion {
                fuente: "youtube".into(),
                id: "song-id".into(),
                nombre: "Song".into(),
                artista: "Artist".into(),
                url: "https://youtube.com/watch?v=song-id".into(),
                stream_url: Some("https://audio.example/song".into()),
                es_directo: false,
                duracion: 120,
                miniatura: None,
                autor_url: None,
                edad_restringida: false,
                es_letra: false,
            },
            None,
        );
        current.track_id = "track-contract-1".into();
        queue.canciones.push(current);
    }
    let (base, _server) = spawn_api(ctx).await;
    let response = Client::new()
        .get(format!("{base}/api/v1/guilds/{GUILD}/player"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/player-active.json"
    ))
    .unwrap();
    assert_eq!(body, expected);
}

#[tokio::test]
async fn test_play_validations_snowflake_ssrf_and_unsupported() {
    let (base, _h) = spawn_api(test_context()).await;
    let client = Client::new();

    // 1. Invalid snowflake
    let bad_sf = client
        .post(format!("{base}/api/v1/guilds/not-a-snowflake/play"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({"query": "lofi"}))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_sf.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        bad_sf.json::<Value>().await.unwrap()["code"],
        "INVALID_SNOWFLAKE"
    );

    // 2. SSRF blocked targets
    for target in &[
        "http://127.0.0.1:3000",
        "http://localhost/x",
        "http://169.254.169.254/meta",
        "file:///etc/passwd",
        "data:text/plain,x",
    ] {
        let res = client
            .post(format!("{base}/api/v1/guilds/{GUILD}/play"))
            .bearer_auth(SECRET)
            .header("x-user-discord-id", USER)
            .json(&json!({ "query": target }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        assert_eq!(res.json::<Value>().await.unwrap()["code"], "BLOCKED_URL");
    }

    // 3. SoundCloud unsupported URL
    let sc = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/play"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({
            "query": "https://soundcloud.com/artist/track"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(sc.status(), StatusCode::BAD_REQUEST);
    assert_eq!(sc.json::<Value>().await.unwrap()["code"], "UNSUPPORTED_URL");
}

#[tokio::test]
async fn play_query_limit_and_dev_mode_inputs_match_node() {
    let flags = Arc::new(std::sync::Mutex::new(Vec::new()));
    let ctx = test_context().con_proveedor(Arc::new(ProveedorPlayFlags(flags.clone())));
    let (base, _server) = spawn_api(ctx).await;
    let client = Client::new();
    let url = format!("{base}/api/v1/guilds/{GUILD}/play");
    let too_long = client
        .post(&url)
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({"query": "x".repeat(1001)}))
        .send()
        .await
        .unwrap();
    assert_eq!(too_long.status(), StatusCode::BAD_REQUEST);
    let body: Value = too_long.json().await.unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/play-query-too-long.json"
    ))
    .unwrap();
    assert_eq!(
        json!({"status": 400, "code": body["code"], "detail": body["detail"]}),
        expected
    );
    assert!(flags.lock().unwrap().is_empty());

    let body_flag = client
        .post(&url)
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({"query": "lofi", "devMode": true}))
        .send()
        .await
        .unwrap();
    assert_eq!(body_flag.status(), StatusCode::OK);
    let header_flag = client
        .post(&url)
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .header("x-dev-mode", "true")
        .json(&json!({"query": "lofi"}))
        .send()
        .await
        .unwrap();
    assert_eq!(header_flag.status(), StatusCode::OK);
    assert_eq!(*flags.lock().unwrap(), vec![false, true]);
}

#[tokio::test]
async fn test_control_actions_and_invalid_action() {
    let (base, _h) = spawn_api(test_context()).await;
    let client = Client::new();

    let bad = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/control"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({"action": "dance"}))
        .send()
        .await
        .unwrap();
    let bad_status = bad.status().as_u16();
    let bad_body: Value = bad.json().await.unwrap();
    let invalid_action: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/control-invalid-action.json"
    ))
    .unwrap();
    assert_eq!(
        json!({"status": bad_status, "code": bad_body["code"]}),
        invalid_action
    );

    let no_queue_fixture: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/control-no-queue.json"
    ))
    .unwrap();

    for act in &["pause", "resume", "skip", "volume", "loop", "shuffle"] {
        let res = client
            .post(format!("{base}/api/v1/guilds/{GUILD}/control"))
            .bearer_auth(SECRET)
            .header("x-user-discord-id", USER)
            .json(&json!({ "action": act }))
            .send()
            .await
            .unwrap();
        let status = res.status().as_u16();
        let body: Value = res.json().await.unwrap();
        assert_eq!(
            json!({"status": status, "code": body["code"]}),
            no_queue_fixture
        );
    }
}

#[tokio::test]
async fn test_dev_mode_toggle_and_retrieval() {
    let (base, _h) = spawn_api(test_context()).await;
    let client = Client::new();

    let get1 = client
        .get(format!("{base}/api/v1/user/dev-mode"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .send()
        .await
        .unwrap();
    assert_eq!(get1.status(), StatusCode::OK);
    let j1: Value = get1.json().await.unwrap();
    assert_eq!(j1["success"], true);
    assert_eq!(j1["devMode"], false);

    let post1 = client
        .post(format!("{base}/api/v1/user/dev-mode"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({"enabled": true}))
        .send()
        .await
        .unwrap();
    assert_eq!(post1.status(), StatusCode::OK);
    let j2: Value = post1.json().await.unwrap();
    assert_eq!(j2["success"], true);
    assert_eq!(j2["devMode"], true);

    let get2 = client
        .get(format!("{base}/api/v1/user/dev-mode"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .send()
        .await
        .unwrap();
    assert_eq!(get2.status(), StatusCode::OK);
    assert_eq!(get2.json::<Value>().await.unwrap()["devMode"], true);
}
