//! Pruebas de ajustes de voz, favoritos y playlists con Supabase (Spec S-30).

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use bot::datos::ajustes_voz::{sanitize_wake_word, GestorAjustesVoz};
use bot::datos::favoritos::{AccionFavorito, CancionFavorito, GestorFavoritos};
use bot::datos::listas::{GestorListas, UserPlaylistInput};
use bot::datos::ClienteSupabaseRest;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

#[derive(Default, Clone)]
struct EstadoMock {
    voice_upsert_fails: Arc<Mutex<bool>>,
    fav_db: Arc<Mutex<Vec<Value>>>,
    pl_db: Arc<Mutex<Vec<Value>>>,
}

async fn mock_server(estado: EstadoMock) -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route("/rest/v1/guild_voice_settings", get(|| async {
            let fix: Value = serde_json::from_str(include_str!("fixtures/supabase/voice_settings.json")).unwrap();
            (StatusCode::OK, Json(fix))
        }).post(|State(st): State<EstadoMock>, Json(b): Json<Value>| async move {
            if *st.voice_upsert_fails.lock().unwrap() {
                (StatusCode::INTERNAL_SERVER_ERROR, "error").into_response()
            } else {
                (StatusCode::CREATED, Json(vec![b])).into_response()
            }
        }).delete(|| async { (StatusCode::OK, Json(json!([]))) }))
        .route("/rest/v1/guild_voice_bindings", get(|| async {
            let fix: Value = serde_json::from_str(include_str!("fixtures/supabase/voice_bindings.json")).unwrap();
            (StatusCode::OK, Json(fix))
        }).post(|Json(b): Json<Value>| async move { (StatusCode::CREATED, Json(b)) })
        .delete(|| async { (StatusCode::OK, Json(json!([]))) }))
        .route("/rest/v1/user_favorites", get(|State(st): State<EstadoMock>, Query(q): Query<HashMap<String, String>>| async move {
            let sid = q.get("song_id").map(|s| s.trim_start_matches("eq."));
            let list = st.fav_db.lock().unwrap().clone();
            Json(list.into_iter().filter(|v| sid.is_none() || v.get("song_id").and_then(Value::as_str) == sid).collect::<Vec<_>>())
        }).post(|State(st): State<EstadoMock>, Json(mut b): Json<Value>| async move {
            b["id"] = json!("fav-created-1");
            st.fav_db.lock().unwrap().push(b.clone());
            (StatusCode::CREATED, Json(vec![b]))
        }).delete(|State(st): State<EstadoMock>| async move {
            st.fav_db.lock().unwrap().clear();
            (StatusCode::OK, Json(json!([])))
        }))
        .route("/rest/v1/user_playlists", get(|State(st): State<EstadoMock>| async move {
            Json(st.pl_db.lock().unwrap().clone())
        }).post(|State(st): State<EstadoMock>, Json(mut b): Json<Value>| async move {
            b["id"] = json!("pl-created-1");
            st.pl_db.lock().unwrap().push(b.clone());
            (StatusCode::CREATED, Json(vec![b]))
        }).patch(|State(st): State<EstadoMock>, Json(b): Json<Value>| async move {
            let mut list = st.pl_db.lock().unwrap();
            if let Some(f) = list.first_mut() { *f = b.clone(); }
            (StatusCode::OK, Json(vec![b]))
        }))
        .with_state(estado);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), handle)
}

#[test]
fn test_sanitize_wake_word_spec_cases() {
    assert_eq!(sanitize_wake_word("bot"), "ignite");
    assert_eq!(sanitize_wake_word("Ignite, bot"), "ignite");
    assert_eq!(sanitize_wake_word(""), "ignite");
    assert_eq!(sanitize_wake_word("   "), "ignite");
    assert_eq!(sanitize_wake_word("bot mierda"), "bot mierda");
    assert_eq!(sanitize_wake_word("mierdabot"), "mierdabot");
    assert_eq!(sanitize_wake_word("Ignite, botcito"), "ignite, botcito");
}

#[tokio::test]
async fn test_ajustes_voz_defecto_y_cache_offline() {
    let gestor = GestorAjustesVoz::nuevo(None, false, "ignite".into(), false);
    let def = gestor.obtener_defecto();
    assert_eq!(def.wake_word, "ignite");
    assert_eq!(def.bindings.len(), 0);

    let g1 = gestor.get_guild_settings("123456").await;
    assert_eq!(g1.wake_word, "ignite");

    let upd = gestor
        .update_guild_settings(
            "123456",
            &json!({
                "wake_word": "botcito",
                "bindings": [{ "phrase": "ponte esta", "command_id": "play" }]
            }),
        )
        .await
        .unwrap();
    assert_eq!(upd.wake_word, "botcito");
    assert_eq!(upd.bindings.len(), 1);

    // Caché en memoria devuelve el nuevo valor
    let cached = gestor.get_guild_settings("123456").await;
    assert_eq!(cached.wake_word, "botcito");

    // Reinicio
    let rst = gestor.reset_guild_settings("123456").await;
    assert_eq!(rst.wake_word, "ignite");
    assert_eq!(rst.bindings.len(), 0);
}

#[tokio::test]
async fn test_ajustes_voz_upsert_fallido_503_sin_cambio_cache() {
    let estado = EstadoMock::default();
    *estado.voice_upsert_fails.lock().unwrap() = true;
    let (url, _h) = mock_server(estado).await;
    let cliente = Arc::new(ClienteSupabaseRest::nuevo(url, "test-key".into()));
    let gestor = GestorAjustesVoz::nuevo(Some(cliente), false, "ignite".into(), false);

    let err = gestor
        .update_guild_settings(
            "guild_err_1",
            &json!({
                "wake_word": "nuevo_wake",
                "bindings": [{ "phrase": "test", "command_id": "skip" }]
            }),
        )
        .await;

    // Regla S-30: 503 DB_UNAVAILABLE cuando falla el upsert
    assert!(err.is_err());
    let (code, id, _) = err.unwrap_err();
    assert_eq!(code, 503);
    assert_eq!(id, "DB_UNAVAILABLE");

    // La caché NO cambió
    let actual = gestor.get_guild_settings("guild_err_1").await;
    assert_ne!(actual.wake_word, "nuevo_wake");
}

#[tokio::test]
async fn test_ajustes_voz_con_servidor_y_bindings_reemplazo() {
    let estado = EstadoMock::default();
    let (url, _h) = mock_server(estado).await;
    let cliente = Arc::new(ClienteSupabaseRest::nuevo(url, "test-key".into()));
    let gestor = GestorAjustesVoz::nuevo(Some(cliente), false, "ignite".into(), false);

    let cargado = gestor.get_guild_settings("123456789012345678").await;
    assert_eq!(cargado.tts_voice, "es-ES-AlvaroNeural");
    assert_eq!(cargado.tts_pitch, 1.2);
    assert_eq!(cargado.bindings.len(), 2);

    // Reemplazo completo de bindings
    let updated = gestor
        .update_guild_settings(
            "123456789012345678",
            &json!({
                "bindings": [{ "phrase": "cancion nueva", "command_id": "play" }]
            }),
        )
        .await
        .unwrap();
    assert_eq!(updated.bindings.len(), 1);
    assert_eq!(updated.bindings[0].phrase, "cancion nueva");
}

#[tokio::test]
async fn test_favoritos_inserta_y_borra() {
    let estado = EstadoMock::default();
    let (url, _h) = mock_server(estado).await;
    let cliente = Arc::new(ClienteSupabaseRest::nuevo(url, "test-key".into()));
    let gestor = GestorFavoritos::nuevo(Some(cliente));

    let song = CancionFavorito {
        song_id: "s123".into(),
        title: "Test Song".into(),
        artist: "Test Artist".into(),
        album_art: None,
        url: "https://youtube.com/watch?v=123".into(),
        duration: 200,
        source: "youtube".into(),
    };

    // 1. No existe -> Agregado
    let res1 = gestor.handle_favorite("user1", &song).await.unwrap();
    assert_eq!(res1, AccionFavorito::Agregado("Test Song".into()));

    // 2. Existe -> Eliminado
    let res2 = gestor.handle_favorite("user1", &song).await.unwrap();
    assert_eq!(res2, AccionFavorito::Eliminado("Test Song".into()));

    // Sin cliente -> 503 DB_UNAVAILABLE
    let gestor_off = GestorFavoritos::nuevo(None);
    let err = gestor_off
        .handle_favorite("user1", &song)
        .await
        .unwrap_err();
    assert_eq!(err.0, 503);
    assert_eq!(err.1, "DB_UNAVAILABLE");
}

#[tokio::test]
async fn test_playlists_upsert_y_fetch() {
    let estado = EstadoMock::default();
    let (url, _h) = mock_server(estado).await;
    let cliente = Arc::new(ClienteSupabaseRest::nuevo(url, "test-key".into()));
    let gestor = GestorListas::nuevo(Some(cliente));

    let input = UserPlaylistInput {
        user_id: "user1".into(),
        name: Some("Mi Lista".into()),
        url: "https://youtube.com/playlist?list=123".into(),
        source: Some("youtube".into()),
        cover: None,
        tracks: vec![json!({"title": "Song 1"})],
        synced_at: None,
    };

    // Crear
    let up1 = gestor.upsert_user_playlist(&input).await.unwrap();
    assert!(up1.created);
    assert_eq!(up1.playlist.name, "Mi Lista");

    // Actualizar mismo par (user_id, url)
    let up2 = gestor.upsert_user_playlist(&input).await.unwrap();
    assert!(!up2.created);

    // Listar
    let listas = gestor.fetch_user_playlists("user1", 10).await.unwrap();
    assert_eq!(listas.len(), 1);

    // Sin cliente -> 503 DB_UNAVAILABLE
    let gestor_off = GestorListas::nuevo(None);
    let err = gestor_off.upsert_user_playlist(&input).await.unwrap_err();
    assert_eq!(err.0, 503);
    assert_eq!(err.1, "DB_UNAVAILABLE");
}
