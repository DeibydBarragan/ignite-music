use medios::{
    buscar, buscar_cancion, buscar_listas, relacionados, resolver_youtube, youtube_info,
    FuentesError, Limitador, Prioridad, ResultadoResolucion, YoutubeSearchErrorState,
    YtDlpSettings,
};
use std::sync::{Arc, Mutex};

#[test]
fn test_youtube_info_metadata_and_live() {
    let video_json = serde_json::json!({
        "id": "dQw4w9WgXcQ",
        "title": "Rick Astley - Never Gonna Give You Up (Official Music Video)",
        "duration": 213,
        "channel": "Rick Astley",
        "age_limit": 18
    });
    let info = youtube_info(&video_json);
    assert_eq!(info.id, "dQw4w9WgXcQ");
    assert_eq!(info.nombre, "Never Gonna Give You Up");
    assert_eq!(info.artista, "Rick Astley");
    assert!(info.edad_restringida);
    assert_eq!(info.url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    assert_eq!(
        info.miniatura,
        Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg".to_string())
    );

    let live_json = serde_json::json!({
        "id": "dQw4w9WgXcQ",
        "title": "Live Stream Event",
        "duration": 9999,
        "is_live": true
    });
    let live = youtube_info(&live_json);
    assert!(live.es_directo);
    assert_eq!(live.duracion, 0);
}

#[tokio::test]
async fn test_resolver_youtube_video_and_playlist() {
    let fake_bin = env!("CARGO_BIN_EXE_fake-ytdlp");
    let limiter = Limitador::new(2);
    let settings = YtDlpSettings::default();

    // Resolving video
    let res = resolver_youtube(
        fake_bin,
        "https://youtu.be/dQw4w9WgXcQ",
        None,
        &settings,
        &[],
        &limiter,
        100,
    )
    .await
    .expect("resolve video");

    match res {
        ResultadoResolucion::Cancion(s) => {
            assert_eq!(s.id, "abcdefghijk");
            assert_eq!(s.nombre, "Echo");
        }
        _ => panic!("Expected Cancion"),
    }

    // Resolving playlist
    let res_list = resolver_youtube(
        fake_bin,
        "https://www.youtube.com/playlist?list=PL1",
        None,
        &settings,
        &[],
        &limiter,
        100,
    )
    .await
    .expect("resolve playlist");

    match res_list {
        ResultadoResolucion::Lista(l) => {
            assert_eq!(l.id, "PL1");
            assert_eq!(l.nombre, "My list");
            // Entries with deleted/private are filtered: aaaaaaaaaaa and ccccccccccc remain
            assert_eq!(l.canciones.len(), 2);
            assert_eq!(l.canciones[0].id, "aaaaaaaaaaa");
            assert_eq!(l.canciones[1].id, "ccccccccccc");
        }
        _ => panic!("Expected Lista"),
    }

    // Empty playlist error
    let err_empty = resolver_youtube(
        fake_bin,
        "https://www.youtube.com/playlist?list=PL_EMPTY",
        None,
        &settings,
        &[],
        &limiter,
        100,
    )
    .await
    .unwrap_err();

    assert_eq!(err_empty, FuentesError::ListaVacia);
}

#[tokio::test]
async fn test_buscar_cancion_and_error_reporting() {
    let fake_bin = env!("CARGO_BIN_EXE_fake-ytdlp");
    let limiter = Limitador::new(2);
    let settings = YtDlpSettings::default();
    let error_state = Arc::new(Mutex::new(YoutubeSearchErrorState {
        last_search_error: None,
        last_search_error_at: 0,
    }));

    let found = buscar_cancion(
        fake_bin,
        "never gonna",
        None,
        &settings,
        &[],
        &limiter,
        Prioridad::Alta,
        Some(&error_state),
        None,
    )
    .await
    .expect("buscar_cancion ok");

    assert!(found.is_some());
    let song = found.unwrap();
    assert_eq!(song.id, "dQw4w9WgXcQ");
    assert_eq!(song.nombre, "Never Gonna Give You Up");

    // Search failure updates last_search_error
    let err = buscar_cancion(
        fake_bin,
        "fail_bot",
        None,
        &settings,
        &[],
        &limiter,
        Prioridad::Alta,
        Some(&error_state),
        None,
    )
    .await
    .unwrap_err();

    assert!(matches!(err, FuentesError::YtDlp(_)));
    let state = error_state.lock().unwrap();
    assert!(state.last_search_error.is_some());
    assert!(state.last_search_error_at > 0);
}

#[tokio::test]
async fn test_buscar_buscar_listas_and_relacionados() {
    let fake_bin = env!("CARGO_BIN_EXE_fake-ytdlp");
    let limiter = Limitador::new(2);
    let settings = YtDlpSettings::default();

    let results = buscar(fake_bin, "my query", 5, None, &settings, &[], &limiter)
        .await
        .expect("buscar");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "aaaaaaaaaaa");

    let playlists = buscar_listas(fake_bin, "dance mix", 5, None, &settings, &[], &limiter)
        .await
        .expect("buscar_listas");

    assert_eq!(playlists.len(), 1);
    assert_eq!(playlists[0].id, "PL_FOUND");
    assert_eq!(playlists[0].autor, "Curator");
    assert_eq!(playlists[0].cantidad_pistas, 37);
    let limited = buscar_listas(fake_bin, "limit check", 1, None, &settings, &[], &limiter)
        .await
        .unwrap();
    assert_eq!(limited.len(), 1);
    assert_eq!(limited[0], playlists[0]);

    let related = relacionados(fake_bin, "dQw4w9WgXcQ", None, &settings, &[], &limiter)
        .await
        .expect("relacionados");

    // Self (dQw4w9WgXcQ), short (<60s: bbbbbbbbbbb), long (>900s: ccccccccccc) excluded
    assert_eq!(related.len(), 2);
    assert_eq!(related[0].id, "aaaaaaaaaaa");
    assert_eq!(related[1].id, "ddddddddddd");
}
