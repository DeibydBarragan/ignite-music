//! Tests de integración para rutas REST v1 adicionales (Spec S-27).

use bot::api::contexto::EstadoApiContexto;
use bot::api::crear_router_con_contexto;
use bot::api::formas::ResultadoPlaylistFrontend;
use bot::api::formas::{BotVoiceStatus, EstadoVozRespuesta, UserVoiceStatus};
use nucleo::Config;
use player::GestorColas;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

struct FalloSalida;

struct ProveedorEstadoVoz(EstadoVozRespuesta);

struct ProveedorPlaylistVacia;

struct ProveedorDiscoveryVacio;

#[async_trait::async_trait]
impl bot::api::contexto::ProveedorApi for ProveedorDiscoveryVacio {
    async fn recomendaciones(
        &self,
        _: &str,
        force: bool,
    ) -> Result<bot::api::formas::RecomendacionRespuesta, String> {
        if force {
            return Ok(bot::api::formas::RecomendacionRespuesta::default());
        }
        Ok(bot::api::formas::RecomendacionRespuesta {
            source: "top".into(),
            based_on: None,
            songs: bot::api::formas::CancionesRecomendadas::Ranking(Vec::new()),
        })
    }
}

#[async_trait::async_trait]
impl bot::api::contexto::ProveedorApi for ProveedorPlaylistVacia {
    async fn resolver_playlist(
        &self,
        _: &str,
        _: &str,
    ) -> Result<ResultadoPlaylistFrontend, String> {
        Ok(ResultadoPlaylistFrontend {
            success: true,
            kind: "playlist".into(),
            name: "Empty playlist".into(),
            source: "youtube".into(),
            count: 0,
            truncated: false,
            tracks: Vec::new(),
        })
    }
}

#[async_trait::async_trait]
impl bot::api::contexto::ProveedorApi for ProveedorEstadoVoz {
    async fn estado_voz(&self, _: &str, _: Option<&str>) -> Result<EstadoVozRespuesta, String> {
        Ok(self.0.clone())
    }
}

#[async_trait::async_trait]
impl bot::api::contexto::ProveedorApi for FalloSalida {
    async fn salir_voz(&self, _: &str) -> Result<(), String> {
        Err("driver indisponible".into())
    }
}

const SECRET: &str = "test-secret-0123456789abcdef0123456789abcdef";
const GUILD: &str = "123456789012345678";
const USER: &str = "987654321098765432";

fn test_context() -> EstadoApiContexto {
    let mut cfg = Config::from_map(
        std::path::PathBuf::from("."),
        &std::collections::HashMap::new(),
    );
    cfg.api.secret = SECRET.to_string();
    cfg.api.enabled = true;
    cfg.data_dir = std::env::temp_dir().join(format!("ignite_adic_test_{}", fastrand::u64(..)));
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
async fn test_guilds_endpoint() {
    let (base, _h) = spawn_api(test_context()).await;
    let client = Client::new();

    let res = client
        .get(format!("{base}/api/v1/guilds"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let j: Value = res.json().await.unwrap();
    assert!(j["guilds"].is_array());
}

#[tokio::test]
async fn test_top_songs_endpoint() {
    let (base, _h) = spawn_api(test_context()).await;
    let client = Client::new();

    let bad_sf = client
        .get(format!("{base}/api/v1/guilds/not-a-snowflake/top-songs"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(bad_sf.status(), StatusCode::BAD_REQUEST);

    let bad_time = client
        .get(format!(
            "{base}/api/v1/guilds/{GUILD}/top-songs?timeframe=year"
        ))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(bad_time.status(), StatusCode::BAD_REQUEST);

    let ok = client
        .get(format!(
            "{base}/api/v1/guilds/{GUILD}/top-songs?timeframe=week&limit=10"
        ))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
    let j: Value = ok.json().await.unwrap();
    assert_eq!(j["success"], true);
    assert_eq!(j["guildId"], GUILD);
    assert_eq!(j["timeframe"], "week");
    assert!(j["songs"].is_array());
}

#[tokio::test]
async fn test_voice_status_and_channels_endpoints() {
    let (base, _h) = spawn_api(test_context()).await;
    let client = Client::new();

    let bad_user = client
        .get(format!(
            "{base}/api/v1/guilds/{GUILD}/voice-status?userId=invalid"
        ))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(bad_user.status(), StatusCode::BAD_REQUEST);

    let whitespace_user = client
        .get(format!(
            "{base}/api/v1/guilds/{GUILD}/voice-status?userId=+++"
        ))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(whitespace_user.status(), StatusCode::BAD_REQUEST);

    let status_ok = client
        .get(format!(
            "{base}/api/v1/guilds/{GUILD}/voice-status?userId={USER}"
        ))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(status_ok.status(), StatusCode::OK);
    let j_status: Value = status_ok.json().await.unwrap();
    assert_eq!(j_status["canControl"], false);

    let bad_chan_sf = client
        .get(format!("{base}/api/v1/guilds/invalid/channels"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(bad_chan_sf.status(), StatusCode::BAD_REQUEST);

    let chan_ok = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/channels"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(chan_ok.status(), StatusCode::OK);
    let j_chan: Value = chan_ok.json().await.unwrap();
    assert_eq!(j_chan["guildId"], GUILD);
    assert!(j_chan["channels"].is_array());
}

#[tokio::test]
async fn test_recommendations_and_search_playlists() {
    let (base, _h) = spawn_api(test_context()).await;
    let client = Client::new();

    let rec_ok = client
        .get(format!("{base}/api/v1/guilds/{GUILD}/recommendations"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(rec_ok.status(), StatusCode::OK);
    let j_rec: Value = rec_ok.json().await.unwrap();
    assert_eq!(j_rec["success"], true);
    assert!(j_rec["songs"].is_array());

    let pl_ok = client
        .get(format!("{base}/api/v1/search/playlists?q=rock&limit=5"))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(pl_ok.status(), StatusCode::OK);
    let j_pl: Value = pl_ok.json().await.unwrap();
    assert_eq!(j_pl["query"], "rock");
    assert!(j_pl["results"].is_array());
}

#[tokio::test]
async fn test_voice_join_and_leave_endpoints() {
    let (base, _h) = spawn_api(test_context()).await;
    let client = Client::new();

    let join_ok = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/voice/join"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .json(&json!({
            "channelId": "111222333444555666"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(join_ok.status(), StatusCode::OK);
    let j_join: Value = join_ok.json().await.unwrap();
    assert_eq!(j_join["success"], true);

    let leave_ok = client
        .post(format!("{base}/api/v1/guilds/{GUILD}/voice/leave"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .send()
        .await
        .unwrap();
    assert_eq!(leave_ok.status(), StatusCode::OK);
    let j_leave: Value = leave_ok.json().await.unwrap();
    assert_eq!(j_leave["success"], true);
}

#[tokio::test]
async fn voice_leave_informa_el_error_del_driver() {
    let ctx = test_context().con_proveedor(Arc::new(FalloSalida));
    let (base, _h) = spawn_api(ctx).await;
    let response = Client::new()
        .post(format!("{base}/api/v1/guilds/{GUILD}/voice/leave"))
        .bearer_auth(SECRET)
        .header("x-user-discord-id", USER)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["code"], "voice_leave_failed");
    assert!(!body.to_string().contains("driver indisponible"));
}

#[tokio::test]
async fn voice_status_control_permission_matches_node_policy() {
    let user_only = EstadoVozRespuesta {
        bot: None,
        user: Some(UserVoiceStatus {
            id: "111222333444555666".into(),
            name: "General".into(),
        }),
        can_control: true,
    };
    let normal_context =
        test_context().con_proveedor(Arc::new(ProveedorEstadoVoz(user_only.clone())));
    let (normal_base, _normal_server) = spawn_api(normal_context).await;
    let client = Client::new();
    let normal = client
        .get(format!(
            "{normal_base}/api/v1/guilds/{GUILD}/voice-status?userId={USER}"
        ))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(normal.json::<Value>().await.unwrap()["canControl"], false);

    let same_channel = EstadoVozRespuesta {
        bot: Some(BotVoiceStatus {
            id: "111222333444555666".into(),
            name: "General".into(),
            deaf: false,
            self_deaf: false,
            server_deaf: false,
        }),
        user: user_only.user.clone(),
        can_control: false,
    };
    let same_context = test_context().con_proveedor(Arc::new(ProveedorEstadoVoz(same_channel)));
    let (same_base, _same_server) = spawn_api(same_context).await;
    let same = client
        .get(format!(
            "{same_base}/api/v1/guilds/{GUILD}/voice-status?userId={USER}"
        ))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(same.json::<Value>().await.unwrap()["canControl"], true);

    let dev_context = test_context().con_proveedor(Arc::new(ProveedorEstadoVoz(user_only)));
    dev_context.dev_mode.establecer_dev_mode(USER, true);
    let (dev_base, _dev_server) = spawn_api(dev_context).await;
    let dev = client
        .get(format!(
            "{dev_base}/api/v1/guilds/{GUILD}/voice-status?userId={USER}"
        ))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(dev.json::<Value>().await.unwrap()["canControl"], true);
}

#[tokio::test]
async fn discovery_and_playlist_payloads_match_shared_contract_fixtures() {
    let ctx = test_context().con_proveedor(Arc::new(ProveedorDiscoveryVacio));
    let (base, _server) = spawn_api(ctx).await;
    let client = Client::new();
    let cases = [
        (
            format!("{base}/api/v1/guilds/{GUILD}/voice-status?userId={USER}"),
            "discovery-voice-status-empty.json",
        ),
        (
            format!("{base}/api/v1/guilds/{GUILD}/channels"),
            "discovery-channels-empty.json",
        ),
        (
            format!("{base}/api/v1/guilds/{GUILD}/recommendations"),
            "discovery-recommendations-empty.json",
        ),
        (
            format!("{base}/api/v1/guilds/{GUILD}/top-songs?timeframe=week"),
            "discovery-top-songs-empty.json",
        ),
    ];
    for (url, fixture) in cases {
        let response = client.get(url).bearer_auth(SECRET).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let actual: Value = response.json().await.unwrap();
        let expected: Value = serde_json::from_str(include_str!(
            "../../../../test/fixtures/api-contract/discovery-voice-status-empty.json"
        ))
        .unwrap_or(Value::Null);
        let expected: Value = match fixture {
            "discovery-voice-status-empty.json" => expected,
            "discovery-channels-empty.json" => serde_json::from_str(include_str!(
                "../../../../test/fixtures/api-contract/discovery-channels-empty.json"
            ))
            .unwrap(),
            "discovery-recommendations-empty.json" => serde_json::from_str(include_str!(
                "../../../../test/fixtures/api-contract/discovery-recommendations-empty.json"
            ))
            .unwrap(),
            _ => serde_json::from_str(include_str!(
                "../../../../test/fixtures/api-contract/discovery-top-songs-empty.json"
            ))
            .unwrap(),
        };
        assert_eq!(actual, expected, "contract fixture {fixture}");
    }

    for query in ["force=true", "refresh=true", "force=false"] {
        let body: Value = client
            .get(format!(
                "{base}/api/v1/guilds/{GUILD}/recommendations?{query}"
            ))
            .bearer_auth(SECRET)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            body["source"],
            if query.ends_with("true") {
                "empty"
            } else {
                "top"
            }
        );
    }

    let playlist_context = test_context().con_proveedor(Arc::new(ProveedorPlaylistVacia));
    let (playlist_base, _playlist_server) = spawn_api(playlist_context).await;
    let response = client.get(format!("{playlist_base}/api/v1/guilds/{GUILD}/playlist/resolve?url=https%3A%2F%2Fyoutube.com%2Fplaylist%3Flist%3DPLtest"))
        .bearer_auth(SECRET).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let actual: Value = response.json().await.unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/playlist-resolve-empty.json"
    ))
    .unwrap();
    assert_eq!(actual, expected);

    let unsupported = client
        .get(format!(
            "{base}/api/v1/guilds/{GUILD}/playlist/resolve?url=https%3A%2F%2Fsoundcloud.com%2Fset"
        ))
        .bearer_auth(SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(unsupported.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let actual: Value = unsupported.json().await.unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/playlist-unsupported.json"
    ))
    .unwrap();
    assert_eq!(actual["status"], expected["status"]);
    assert_eq!(actual["code"], expected["code"]);
}
