//! Pruebas de historial, exclusiones UTC y ranking con Supabase (Spec S-30).

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use bot::datos::historial::{
    ExclusionesHistorial, GestorHistorial, RegistroCancionInput, RegistroReproduccion,
};
use bot::datos::ranking::GestorRanking;
use bot::datos::ClienteSupabaseRest;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

#[derive(Default, Clone)]
struct EstadoMock {
    rpc_fails: Arc<Mutex<bool>>,
    song_plays: Arc<Mutex<Vec<Value>>>,
}

async fn mock_server(estado: EstadoMock) -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route(
            "/rest/v1/rpc/top_songs",
            post(|State(st): State<EstadoMock>| async move {
                if *st.rpc_fails.lock().unwrap() {
                    (StatusCode::INTERNAL_SERVER_ERROR, "rpc error".to_string())
                } else {
                    let fix: Value =
                        serde_json::from_str(include_str!("fixtures/supabase/top_songs_rpc.json"))
                            .unwrap();
                    (StatusCode::OK, fix.to_string())
                }
            }),
        )
        .route(
            "/rest/v1/song_plays",
            get(|| async {
                let fix: Value =
                    serde_json::from_str(include_str!("fixtures/supabase/song_plays_scan.json"))
                        .unwrap();
                (StatusCode::OK, Json(fix))
            })
            .post(
                |State(st): State<EstadoMock>, Json(body): Json<Value>| async move {
                    st.song_plays.lock().unwrap().push(body);
                    (StatusCode::CREATED, Json(json!([])))
                },
            ),
        )
        .with_state(estado);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), handle)
}

#[tokio::test]
async fn test_historial_record_play_campos_y_modo_dev() {
    let tmp = tempfile::tempdir().unwrap();
    let excl_file = tmp.path().join("exclusions.json");
    std::fs::write(&excl_file, r#"{"dates": ["2026-10-18"]}"#).unwrap();
    let exclusiones = Arc::new(ExclusionesHistorial::nuevo(excl_file));

    let estado = EstadoMock::default();
    let (url, _h) = mock_server(estado.clone()).await;
    let cliente = Arc::new(ClienteSupabaseRest::nuevo(url, "test-key".into()));
    let gestor = GestorHistorial::nuevo(Some(cliente), tmp.path(), exclusiones);

    let cancion = RegistroCancionInput {
        id: Some("id1".into()),
        title: Some("Song 1".into()),
        artist: Some("Artist 1".into()),
        album_art: Some("https://example.com/art.jpg".into()),
        url: Some("https://youtube.com/watch?v=1".into()),
        duration: Some(180),
        source: Some("youtube".into()),
        user_id: Some("user1".into()),
        username: Some("TestUser".into()),
        dev_mode: false,
    };

    // Play normal
    gestor.record_play("guild_1", &cancion, false).await;
    let local = gestor.cargar_local("guild_1");
    assert_eq!(local.len(), 1);
    assert_eq!(local[0].title, "Song 1");
    assert_eq!(local[0].user_id.as_deref(), Some("user1"));
    assert_eq!(estado.song_plays.lock().unwrap().len(), 1);

    // Play con modo dev activo para el usuario -> no escribe
    gestor.record_play("guild_1", &cancion, true).await;
    assert_eq!(gestor.cargar_local("guild_1").len(), 1);
    assert_eq!(estado.song_plays.lock().unwrap().len(), 1);

    // Play con canción marcada con dev_mode -> no escribe
    let mut dev_song = cancion.clone();
    dev_song.dev_mode = true;
    gestor.record_play("guild_1", &dev_song, false).await;
    assert_eq!(gestor.cargar_local("guild_1").len(), 1);
    assert_eq!(estado.song_plays.lock().unwrap().len(), 1);
}

#[test]
fn test_exclusiones_utc_y_zona_horaria_2330() {
    let tmp = tempfile::tempdir().unwrap();
    let excl_file = tmp.path().join("exclusions.json");
    std::fs::write(&excl_file, r#"{"dates": ["2026-10-18"]}"#).unwrap();
    let exclusiones = ExclusionesHistorial::nuevo(excl_file);

    // 23:30 UTC cae exactamente en 2026-10-18
    let play_2330_utc = "2026-10-18T23:30:00.000Z";
    assert!(exclusiones.es_excluido(play_2330_utc));

    // Día siguiente 00:05 UTC no está excluido
    let play_next_day = "2026-10-19T00:05:00.000Z";
    assert!(!exclusiones.es_excluido(play_next_day));
}

#[tokio::test]
async fn test_ranking_rpc_scan_fallback_y_local_fallback() {
    let tmp = tempfile::tempdir().unwrap();
    let excl_file = tmp.path().join("exclusions.json");
    std::fs::write(&excl_file, r#"{"dates": ["2026-10-14"]}"#).unwrap();
    let exclusiones = Arc::new(ExclusionesHistorial::nuevo(excl_file.clone()));

    let estado = EstadoMock::default();
    let (url, _h) = mock_server(estado.clone()).await;
    let cliente = Arc::new(ClienteSupabaseRest::nuevo(url, "test-key".into()));
    let gestor_h = Arc::new(GestorHistorial::nuevo(
        Some(cliente.clone()),
        tmp.path(),
        exclusiones.clone(),
    ));
    let ranking = GestorRanking::nuevo(Some(cliente.clone()), gestor_h.clone());

    // 1. Respuesta de la función RPC top_songs
    let top_rpc = ranking.get_top_songs("123456", "week", 10).await;
    assert_eq!(top_rpc.len(), 2);
    assert_eq!(top_rpc[0].title, "Never Gonna Give You Up");
    assert_eq!(top_rpc[0].play_count, 42);
    assert_eq!(top_rpc[0].rank, 1);
    assert_eq!(top_rpc[0].formatted_duration, "3:32");
    assert_eq!(top_rpc[1].title, "Together Forever");
    assert_eq!(top_rpc[1].rank, 2);

    // 2. Fallo de la función RPC -> Escaneo acotado en song_plays
    *estado.rpc_fails.lock().unwrap() = true;
    let top_scan = ranking.get_top_songs("123456789012345678", "all", 10).await;
    assert_eq!(top_scan.len(), 2);
    // Track 1 tuvo 2 reproducciones, Track 2 tuvo 1
    assert_eq!(top_scan[0].title, "Cancion A");
    assert_eq!(top_scan[0].play_count, 2);
    assert_eq!(top_scan[1].title, "Cancion B");
    assert_eq!(top_scan[1].play_count, 1);

    // 3. Fallback al historial local si no hay cliente
    let gestor_local = GestorHistorial::nuevo(None, tmp.path(), exclusiones.clone());
    let local_list = vec![RegistroReproduccion {
        guild_id: "local_guild".into(),
        song_id: "loc1".into(),
        title: "Local Track".into(),
        artist: "Local Artist".into(),
        album_art: None,
        url: "https://youtube.com/watch?v=loc".into(),
        duration: 120,
        source: "youtube".into(),
        user_id: None,
        username: None,
        played_at: "2026-10-18T10:00:00.000Z".to_string(),
    }];
    gestor_local.guardar_local("local_guild", local_list);

    let ranking_local = GestorRanking::nuevo(None, Arc::new(gestor_local));
    let top_loc = ranking_local.get_top_songs("local_guild", "all", 5).await;
    assert_eq!(top_loc.len(), 1);
    assert_eq!(top_loc[0].title, "Local Track");
    assert_eq!(top_loc[0].play_count, 1);
    assert_eq!(top_loc[0].rank, 1);
}

#[tokio::test]
async fn test_ranking_limites_1_a_50() {
    let tmp = tempfile::tempdir().unwrap();
    let excl_file = tmp.path().join("exclusions.json");
    std::fs::write(&excl_file, r#"{"dates": []}"#).unwrap();
    let exclusiones = Arc::new(ExclusionesHistorial::nuevo(excl_file));

    let estado = EstadoMock::default();
    let (url, _h) = mock_server(estado).await;
    let cliente = Arc::new(ClienteSupabaseRest::nuevo(url, "test-key".into()));
    let gestor_h = Arc::new(GestorHistorial::nuevo(
        Some(cliente.clone()),
        tmp.path(),
        exclusiones,
    ));
    let ranking = GestorRanking::nuevo(Some(cliente), gestor_h);

    // Límite solicitado 0 se acota a 1
    let r1 = ranking.get_top_songs("123456", "week", 0).await;
    assert_eq!(r1.len(), 1);

    // Límite solicitado 100 se acota a 50 (el fixture tiene 2, por lo que devuelve 2)
    let r50 = ranking.get_top_songs("123456", "week", 100).await;
    assert_eq!(r50.len(), 2);
}

#[tokio::test]
async fn test_verify_connection() {
    let (url, _h) = mock_server(EstadoMock::default()).await;
    let cliente = ClienteSupabaseRest::nuevo(url, "test-key".into());
    let res = cliente.verify_connection(1000).await;
    assert!(res.ok);
    assert!(res.reason.is_none());

    // Timeout o puerto inaccesible
    let cliente_err = ClienteSupabaseRest::nuevo("http://127.0.0.1:1".into(), "test-key".into());
    let res_err = cliente_err.verify_connection(200).await;
    assert!(!res_err.ok);
    assert!(res_err.reason.is_some());
}

#[tokio::test]
async fn ranking_http_y_recomendaciones_conservan_metadatos_de_node() {
    use bot::api::contexto::EstadoApiContexto;
    use bot::api::proveedor_real::ProveedorApiReal;
    use std::time::Instant;

    let tmp = tempfile::tempdir().unwrap();
    let (url, _mock) = mock_server(EstadoMock::default()).await;
    let cliente = Arc::new(ClienteSupabaseRest::nuevo(url, "test-key".into()));
    let exclusiones = Arc::new(ExclusionesHistorial::nuevo(
        tmp.path().join("exclusions.json"),
    ));
    let historial = Arc::new(GestorHistorial::nuevo(None, tmp.path(), exclusiones));
    let ranking = Arc::new(GestorRanking::nuevo(Some(cliente), historial));

    let mut cfg = nucleo::Config::from_map(tmp.path().into(), &std::collections::HashMap::new());
    cfg.api.enabled = true;
    cfg.api.port = 0;
    cfg.api.secret = "test-secret-0123456789abcdef0123456789abcdef".into();
    let secret = cfg.api.secret.clone();
    let colas = Arc::new(player::GestorColas::nuevo());
    let proveedor = ProveedorApiReal::nuevo(
        Arc::new(twilight_cache_inmemory::InMemoryCache::new()),
        None,
        None,
        colas.clone(),
        Some(ranking),
        0,
        medios::YtDlpSettings::default(),
        Vec::new(),
        None,
        100,
        100,
    );
    let ctx = EstadoApiContexto::nuevo(Arc::new(cfg), colas).con_proveedor(Arc::new(proveedor));
    let server = bot::api::iniciar_servidor_api_con_contexto(ctx, Instant::now())
        .await
        .unwrap()
        .unwrap();
    let base = format!(
        "http://127.0.0.1:{}/api/v1/guilds/123456789012345678",
        server.port
    );
    let client = reqwest::Client::new();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/discovery-catalog.json"
    ))
    .unwrap();

    let cases = [
        ("top-songs?timeframe=all", fixture["topSongs"].clone()),
        (
            "recommendations",
            json!({
                "success":true, "source":"top", "basedOn":null, "songs":fixture["topSongs"]["songs"],
            }),
        ),
    ];
    for (path, expected) in cases {
        let body = client
            .get(format!("{base}/{path}"))
            .bearer_auth(&secret)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
        assert_eq!(body, expected, "{path}");
    }
    server.apagar().await;
}
