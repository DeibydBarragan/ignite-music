use medios::{
    resolver_generica, valida_url_generica, FuentesError, Limitador, ResultadoResolucion,
    YtDlpSettings,
};

#[test]
fn test_valida_url_generica() {
    assert!(valida_url_generica("http://example.com/song.mp3"));
    assert!(valida_url_generica("https://bandcamp.com/track/1"));
    assert!(!valida_url_generica("file:///etc/passwd"));
    assert!(!valida_url_generica("not_a_url"));
}

#[tokio::test]
async fn test_resolver_generica_track_and_playlist() {
    let fake_bin = env!("CARGO_BIN_EXE_fake-ytdlp");
    let limiter = Limitador::new(2);
    let settings = YtDlpSettings::default();

    // Single track resolution
    let res = resolver_generica(
        fake_bin,
        "https://band.bandcamp.com/track/x",
        None,
        &settings,
        &[],
        &limiter,
        100,
    )
    .await
    .expect("resolve bandcamp track");

    match res {
        ResultadoResolucion::Cancion(s) => {
            assert_eq!(s.fuente, "bandcamp");
            assert_eq!(s.id, "1");
            assert_eq!(s.nombre, "Track");
            assert_eq!(s.duracion, 60);
        }
        _ => panic!("Expected Cancion"),
    }

    // Playlist / album resolution
    let res_list = resolver_generica(
        fake_bin,
        "https://example.com/album",
        None,
        &settings,
        &[],
        &limiter,
        100,
    )
    .await
    .expect("resolve album");

    match res_list {
        ResultadoResolucion::Lista(l) => {
            assert_eq!(l.id, "al");
            assert_eq!(l.nombre, "Album");
            assert_eq!(l.canciones.len(), 2);
            assert_eq!(l.canciones[0].id, "1");
            assert_eq!(l.canciones[1].id, "2");
        }
        _ => panic!("Expected Lista"),
    }

    // Invalid url scheme
    let err = resolver_generica(
        fake_bin,
        "ftp://example.com/audio.mp3",
        None,
        &settings,
        &[],
        &limiter,
        100,
    )
    .await
    .unwrap_err();

    assert_eq!(err, FuentesError::UrlNoSoportada);
}
