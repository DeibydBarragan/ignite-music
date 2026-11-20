//! Tests de integración para descubrimiento, voz y paridad con Node (Spec S-27).

use bot::api::contexto::EstadoApiContexto;
use bot::api::crear_router_con_contexto;
use nucleo::Config;
use player::GestorColas;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

const SECRET: &str = "test-secret-0123456789abcdef0123456789abcdef";
const GUILD: &str = "123456789012345678";
const USER: &str = "987654321098765432";

struct ProveedorBusquedaContador(Arc<AtomicUsize>);

#[async_trait::async_trait]
impl bot::api::contexto::ProveedorApi for ProveedorBusquedaContador {
    async fn buscar(
        &self,
        _: &str,
        _: usize,
        _: Option<&str>,
    ) -> Result<Vec<bot::api::formas::PistaFrontend>, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Vec::new())
    }

    async fn sugerir(&self, _: &str) -> Result<Vec<String>, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Vec::new())
    }

    async fn buscar_playlists(
        &self,
        _: &str,
        _: usize,
        _: Option<&str>,
    ) -> Result<Vec<bot::api::formas::PlaylistFrontend>, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Vec::new())
    }
}

fn test_context() -> EstadoApiContexto {
    let mut cfg = Config::from_map(
        std::path::PathBuf::from("."),
        &std::collections::HashMap::new(),
    );
    cfg.api.secret = SECRET.to_string();
    cfg.api.enabled = true;
    cfg.data_dir = std::env::temp_dir().join(format!("ignite_desc_test_{}", fastrand::u64(..)));
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
async fn test_voice_settings_get_update_reset_and_logs() {
    let (base, _h) = spawn_api(test_context()).await;
    let client = Client::new();

    let get1 = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/voice"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(get1.status(), StatusCode::OK);
    assert_eq!(get1.json::<Value>().await.unwrap()["wake_word"], "ignite");

    let put1 = client
        .put(format!("{base}/api/v1/guilds/{GUILD}/voice"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({"wake_word": "botcito"}))
        .send()
        .await
        .unwrap();
    assert_eq!(put1.status(), StatusCode::OK);
    assert_eq!(put1.json::<Value>().await.unwrap()["wake_word"], "botcito");

    for body in [
        json!({"enabled": "yes"}),
        json!({"tts_pitch": "1"}),
        json!([]),
        json!({"language": "x"}),
        json!({"bindings": [{"phrase": "", "command_id": "pause"}]}),
        json!({"tts_templates": {"pause": false}}),
    ] {
        let invalid = client
            .put(format!("{base}/api/v1/guilds/{GUILD}/voice"))
            .bearer_auth(SECRET)
            .header("x-user-discord-id", USER)
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    }

    let rst = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/voice/reset"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .send()
        .await
        .unwrap();
    assert_eq!(rst.status(), StatusCode::OK);
    assert_eq!(rst.json::<Value>().await.unwrap()["wake_word"], "ignite");

    let logs = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/voice/logs?limit=10"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(logs.status(), StatusCode::OK);
    assert!(logs.json::<Value>().await.unwrap()["logs"].is_array());

    let clr = client
        .delete(format!("{base}/api/v1/guilds/{GUILD}/voice/logs"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .send()
        .await
        .unwrap();
    assert_eq!(clr.status(), StatusCode::OK);
    assert_eq!(clr.json::<Value>().await.unwrap()["ok"], true);
}

#[tokio::test]
async fn test_discovery_search_and_playlist_resolve() {
    let (base, _h) = spawn_api(test_context()).await;
    let client = Client::new();

    let s1 = client
        .get(format!("{base}/api/v1/search?q=lofi"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(s1.status(), StatusCode::OK);
    let js1: Value = s1.json().await.unwrap();
    assert_eq!(js1["query"], "lofi");
    assert!(js1["results"].is_array());

    let sug = client
        .get(format!("{base}/api/v1/search/suggest?q=lofi"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(sug.status(), StatusCode::OK);
    assert_eq!(sug.json::<Value>().await.unwrap()["query"], "lofi");

    let res_block = client
        .get(format!(
            "{base}/api/v1/guilds/{GUILD}/playlist/resolve?url=http://127.0.0.1/pl"
        ))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(res_block.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        res_block.json::<Value>().await.unwrap()["code"],
        "BLOCKED_URL"
    );

    let res_sc = client
        .get(format!(
            "{base}/api/v1/guilds/{GUILD}/playlist/resolve?url=https://soundcloud.com/set"
        ))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(res_sc.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        res_sc.json::<Value>().await.unwrap()["code"],
        "UNSUPPORTED_URL"
    );
}

#[tokio::test]
async fn discovery_query_validation_and_normalization_match_node() {
    let calls = Arc::new(AtomicUsize::new(0));
    let context = test_context().con_proveedor(Arc::new(ProveedorBusquedaContador(calls.clone())));
    let (base, _h) = spawn_api(context).await;
    let client = Client::new();
    let invalid_urls = vec![
        "/search?limit=0".to_string(),
        "/search?limit=16".to_string(),
        "/search?limit=1.5".to_string(),
        "/search?type=unknown".to_string(),
        format!("/search?q={}", "x".repeat(501)),
        format!("/search/suggest?q={}", "x".repeat(501)),
        "/search/playlists?limit=16".to_string(),
        format!("/guilds/{GUILD}/top-songs?limit=101"),
    ];
    for path in invalid_urls {
        let response = client
            .get(format!("{base}/api/v1{path}"))
            .bearer_auth(SECRET)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
        let problem: Value = response.json().await.unwrap();
        assert_eq!(problem["code"], "INVALID_PARAMETER", "{path}");
    }

    let normalized = client
        .get(format!("{base}/api/v1/search?q=+lofi+&limit=1.0"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(normalized.status(), StatusCode::OK);
    let actual: Value = normalized.json().await.unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/discovery-search-normalized.json"
    ))
    .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let unsearchable_queries = vec![
        "a".to_string(),
        "x".repeat(101),
        "https://youtube.com/watch?v=video".to_string(),
    ];
    for query in &unsearchable_queries {
        for path in ["search", "search/playlists", "search/suggest"] {
            let response = client
                .get(format!("{base}/api/v1/{path}?q={query}"))
                .bearer_auth(SECRET)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{path}: {query}");
            let body: Value = response.json().await.unwrap();
            if path == "search/suggest" {
                assert_eq!(body["suggestions"], json!([]), "{path}: {query}");
            } else {
                assert_eq!(body["results"], json!([]), "{path}: {query}");
            }
        }
    }
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "unsearchable queries must not reach the provider"
    );
}

#[tokio::test]
async fn test_node_fixtures_parity() {
    let (base, _h) = spawn_api(test_context()).await;
    let client = Client::new();

    let fixture_raw = std::fs::read_to_string("tests/fixtures/rutas-node.json").unwrap();
    let fixtures: Value = serde_json::from_str(&fixture_raw).unwrap();

    let res_player = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/player"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    let json_player: Value = res_player.json().await.unwrap();
    assert_eq!(
        json_player["hasQueue"],
        fixtures["player_state_empty"]["hasQueue"]
    );
    assert_eq!(
        json_player["volume"],
        fixtures["player_state_empty"]["volume"]
    );

    let res_dev = client
        .get(format!("{base}/api/v1/user/dev-mode"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .send()
        .await
        .unwrap();
    let json_dev: Value = res_dev.json().await.unwrap();
    assert_eq!(json_dev["success"], fixtures["dev_mode_get"]["success"]);

    let res_voice = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/voice"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    let json_voice: Value = res_voice.json().await.unwrap();
    let voice_contract: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/voice-settings-default.json"
    ))
    .unwrap();
    assert_eq!(json_voice, voice_contract);

    let res_blocked = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/play"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({"query": "http://127.0.0.1"}))
        .send()
        .await
        .unwrap();
    let json_blocked: Value = res_blocked.json().await.unwrap();
    assert_eq!(
        json_blocked["code"],
        fixtures["blocked_url_problem"]["code"]
    );
    assert_eq!(
        json_blocked["status"],
        fixtures["blocked_url_problem"]["status"]
    );
}
