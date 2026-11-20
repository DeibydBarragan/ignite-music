use medios::{
    create_search_query, derive_name, detect_source, parse_spotify_url, spotify_track_to_song,
    SpotifyClient, SpotifyUrlType,
};

#[test]
fn test_parse_spotify_url() {
    let track = parse_spotify_url("https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT");
    assert_eq!(
        track,
        Some(SpotifyUrlType::Track("4cOdK2wGLETKBW3PvgPWqT".to_string()))
    );

    let album = parse_spotify_url("https://open.spotify.com/album/1DFixLWuPkv3KT3TnV35m3");
    assert_eq!(
        album,
        Some(SpotifyUrlType::Album("1DFixLWuPkv3KT3TnV35m3".to_string()))
    );

    let playlist = parse_spotify_url("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M");
    assert_eq!(
        playlist,
        Some(SpotifyUrlType::Playlist(
            "37i9dQZF1DXcBWIGoYBM5M".to_string()
        ))
    );

    let uri = parse_spotify_url("spotify:track:4cOdK2wGLETKBW3PvgPWqT");
    assert_eq!(
        uri,
        Some(SpotifyUrlType::Track("4cOdK2wGLETKBW3PvgPWqT".to_string()))
    );

    assert_eq!(parse_spotify_url("https://youtube.com/watch?v=123"), None);
}

#[test]
fn test_create_search_query() {
    assert_eq!(
        create_search_query("Rick Astley", "Never Gonna Give You Up"),
        "Rick Astley - Never Gonna Give You Up"
    );
    assert_eq!(create_search_query("", "Song Only"), "Song Only");
}

#[test]
fn test_detect_source_and_derive_name() {
    assert_eq!(
        detect_source("https://open.spotify.com/track/123"),
        "spotify"
    );
    assert_eq!(
        detect_source("https://www.youtube.com/watch?v=123"),
        "youtube"
    );
    assert_eq!(detect_source("https://youtu.be/123"), "youtube");
    assert_eq!(detect_source("https://soundcloud.com/123"), "generic");

    assert_eq!(
        derive_name("https://example.com/playlist/summer-vibes_2026", None),
        "summer vibes 2026"
    );
    assert_eq!(
        derive_name("https://example.com/p", Some("Mi Lista Favorita")),
        "Mi Lista Favorita"
    );
    assert_eq!(derive_name("invalid url", None), "Nueva playlist");

    let long_name = "a".repeat(100);
    assert_eq!(derive_name("url", Some(&long_name)).len(), 60);
}

#[tokio::test]
async fn test_spotify_client_without_credentials_fails_cleanly() {
    let client = SpotifyClient::new("".to_string(), "".to_string());
    assert!(!client.has_credentials());

    let err = client
        .resolve("https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT")
        .await
        .unwrap_err();

    assert!(err.to_string().contains("not configured"));
}

#[test]
fn test_spotify_track_to_song_explicit_is_not_age_restricted() {
    let raw = serde_json::json!({
        "id": "track123",
        "name": "Explicit Hit",
        "artists": [{ "name": "Artist A" }, { "name": "Artist B" }],
        "duration_ms": 180000,
        "explicit": true,
        "album": {
            "images": [{ "url": "https://img.spotify.com/album.jpg" }]
        }
    });

    let song = spotify_track_to_song(&raw);
    assert_eq!(song.id, "track123");
    assert_eq!(song.nombre, "Explicit Hit");
    assert_eq!(song.artista, "Artist A, Artist B");
    assert_eq!(song.duracion, 180);
    assert_eq!(song.url, "https://open.spotify.com/track/track123");
    assert_eq!(
        song.miniatura,
        Some("https://img.spotify.com/album.jpg".to_string())
    );
    // Explicit is NOT age restricted
    assert!(!song.edad_restringida);
    assert!(!song.es_letra);
}
