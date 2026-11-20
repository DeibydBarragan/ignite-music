use medios::{
    format_duration, resolve_playlist, snapshot_track, Cancion, FuentesError, Limitador,
    YtDlpSettings, FALLBACK_COVER,
};

#[test]
fn test_format_duration_and_snapshot_track() {
    assert_eq!(format_duration(0), "0:00");
    assert_eq!(format_duration(9), "0:09");
    assert_eq!(format_duration(65), "1:05");
    assert_eq!(format_duration(125), "2:05");

    let song_yt = Cancion {
        fuente: "youtube".to_string(),
        id: "abc".to_string(),
        nombre: "Test Video".to_string(),
        artista: "Test Channel".to_string(),
        url: "https://youtube.com/watch?v=abc".to_string(),
        stream_url: Some("https://stream.url".to_string()),
        es_directo: false,
        duracion: 180,
        miniatura: None,
        autor_url: None,
        edad_restringida: false,
        es_letra: true,
    };

    let snap_yt = snapshot_track(&song_yt);
    assert_eq!(snap_yt.id, "abc");
    assert_eq!(snap_yt.title, "Test Video");
    assert_eq!(snap_yt.artist, "Test Channel");
    assert_eq!(snap_yt.album_art, FALLBACK_COVER);
    assert_eq!(snap_yt.duration, 180);
    assert_eq!(snap_yt.formatted_duration, "3:00");
    assert_eq!(snap_yt.source, "youtube");
    assert_eq!(snap_yt.category, "hits");
    assert!(snap_yt.is_lyric);
    assert!(!snap_yt.is_live);

    let song_sp = Cancion {
        fuente: "spotify".to_string(),
        id: "sp123".to_string(),
        nombre: "Spotify Hit".to_string(),
        artista: "Star".to_string(),
        url: "https://open.spotify.com/track/sp123".to_string(),
        stream_url: None,
        es_directo: false,
        duracion: 215,
        miniatura: Some("https://sp.img/cover.jpg".to_string()),
        autor_url: None,
        edad_restringida: false,
        es_letra: false,
    };

    let snap_sp = snapshot_track(&song_sp);
    assert_eq!(snap_sp.source, "spotify");
    assert_eq!(snap_sp.album_art, "https://sp.img/cover.jpg");
    assert_eq!(snap_sp.formatted_duration, "3:35");
}

#[tokio::test]
async fn test_resolve_playlist_youtube() {
    let fake_bin = env!("CARGO_BIN_EXE_fake-ytdlp");
    let limiter = Limitador::new(2);
    let settings = YtDlpSettings::default();

    // Resolving YouTube playlist with custom name
    let res = resolve_playlist(
        "https://www.youtube.com/playlist?list=PL1",
        Some("Mi Lista Personalizada"),
        Some(1),
        None,
        fake_bin,
        None,
        &settings,
        &[],
        &limiter,
    )
    .await
    .expect("resolve playlist ok");

    assert_eq!(res.kind, "playlist");
    assert_eq!(res.name, "Mi Lista Personalizada");
    assert_eq!(res.source, "youtube");
    // Requested limit 1 -> tracks truncated to 1
    assert_eq!(res.tracks.len(), 1);
    assert!(res.truncated);
    assert_eq!(res.tracks[0].id, "aaaaaaaaaaa");

    // Resolving YouTube track
    let res_track = resolve_playlist(
        "https://youtu.be/dQw4w9WgXcQ",
        None,
        None,
        None,
        fake_bin,
        None,
        &settings,
        &[],
        &limiter,
    )
    .await
    .expect("resolve track ok");

    assert_eq!(res_track.kind, "track");
    assert_eq!(res_track.source, "youtube");
    assert_eq!(res_track.tracks.len(), 1);
    assert_eq!(res_track.tracks[0].id, "abcdefghijk");

    // Empty URL error
    let err = resolve_playlist(
        "   ",
        None,
        None,
        None,
        fake_bin,
        None,
        &settings,
        &[],
        &limiter,
    )
    .await
    .unwrap_err();

    assert_eq!(err, FuentesError::UrlNoSoportada);
}

#[tokio::test]
async fn generic_playlist_matches_node_snapshot_at_limit() {
    let settings = YtDlpSettings::default();
    let url = "https://media.example.test/album";
    let resolver = |limit| {
        resolve_playlist(
            url,
            None,
            Some(limit),
            None,
            env!("CARGO_BIN_EXE_fake-ytdlp"),
            None,
            &settings,
            &[],
            &settings.limiter,
        )
    };
    let result = resolver(2).await.unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../test/fixtures/api-contract/discovery-catalog.json"
    ))
    .unwrap();
    assert_eq!(
        serde_json::json!({
            "success":true, "kind":result.kind, "name":result.name, "source":result.source,
            "count":result.tracks.len(), "truncated":result.truncated, "tracks":result.tracks,
        }),
        fixture["playlist"],
    );
    let truncated = resolver(1).await.unwrap();
    assert!(truncated.truncated);
    assert_eq!(truncated.tracks.len(), 1);
}
