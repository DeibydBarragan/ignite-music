use medios::{
    buscar_cancion, prefetch, prefetch_query, stream_info, Cancion, FuentesCache, Limitador,
    Prioridad, YtDlpSettings, CACHE_BUSQUEDA_MAX_SIZE, CACHE_BUSQUEDA_TTL_MS, CACHE_INFOS_MAX_SIZE,
    CACHE_INFOS_TTL_MS, CACHE_LETRAS_MAX_SIZE, CACHE_LETRAS_TTL_MS,
};
use std::fs;
use tempfile::NamedTempFile;

#[test]
fn test_cache_constantes_y_letras() {
    assert_eq!(CACHE_BUSQUEDA_TTL_MS, 30 * 60_000);
    assert_eq!(CACHE_BUSQUEDA_MAX_SIZE, 1000);
    assert_eq!(CACHE_INFOS_TTL_MS, 3_600_000);
    assert_eq!(CACHE_INFOS_MAX_SIZE, 500);
    assert_eq!(CACHE_LETRAS_TTL_MS, 24 * 3_600_000);
    assert_eq!(CACHE_LETRAS_MAX_SIZE, 5000);

    let cache = FuentesCache::new();
    let url = "https://www.youtube.com/watch?v=abc";
    assert!(!cache.is_lyric_song(url));

    cache.set_lyric_song(url, true);
    assert!(cache.is_lyric_song(url));

    cache.set_lyric_song(url, false);
    assert!(!cache.is_lyric_song(url));
}

#[test]
fn test_cache_keys_y_remember_info() {
    let key1 = FuentesCache::info_key(None, "https://youtu.be/123");
    assert_eq!(key1, "direct|https://youtu.be/123");

    let key2 = FuentesCache::info_key(Some("http://proxy:8080"), "https://youtu.be/123");
    assert_eq!(key2, "http://proxy:8080|https://youtu.be/123");

    let skey1 = FuentesCache::search_key(None, "HELLO WORLD");
    assert_eq!(skey1, "direct|hello world");

    let cache = FuentesCache::new();
    let info = serde_json::json!({
        "url": "https://googlevideo.com/playback?expire=1890000000",
        "format_id": "251",
    });
    cache.remember_info(None, "https://youtu.be/test", &info);
    assert!(cache.infos.has("direct|https://youtu.be/test"));
}

#[tokio::test]
async fn test_busquedas_concurrentes_comparten_un_solo_proceso() {
    let fake_bin = env!("CARGO_BIN_EXE_fake-ytdlp");
    let log_file = NamedTempFile::new().expect("temp log file");
    let log_path = log_file.path().to_string_lossy().to_string();

    // Configure fake-ytdlp to write invoked args to log_path
    std::env::set_var("FAKE_YTDLP_LOG", &log_path);
    std::env::set_var("FAKE_YTDLP_LOG_MATCH", "concurrent query");

    let limiter = Limitador::new(2);
    let settings = YtDlpSettings::default();
    let cache = FuentesCache::new();

    // Two identical concurrent searches
    let f1 = buscar_cancion(
        fake_bin,
        "concurrent query",
        Some("guild-1"),
        &settings,
        &[],
        &limiter,
        Prioridad::Alta,
        None,
        Some(&cache),
    );
    let f2 = buscar_cancion(
        fake_bin,
        "concurrent query",
        Some("guild-1"),
        &settings,
        &[],
        &limiter,
        Prioridad::Alta,
        None,
        Some(&cache),
    );

    let (r1, r2) = tokio::join!(f1, f2);
    let s1 = r1.expect("res1").expect("song1");
    let s2 = r2.expect("res2").expect("song2");
    assert_eq!(s1.id, s2.id);

    // Read invocations from log
    let content = fs::read_to_string(&log_path).unwrap_or_default();
    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines.len(),
        1,
        "Dos búsquedas idénticas concurrentes deben generar un solo proceso"
    );

    // Subsequent search should hit the cache immediately without invoking fake-ytdlp
    let r3 = buscar_cancion(
        fake_bin,
        "concurrent query",
        Some("guild-1"),
        &settings,
        &[],
        &limiter,
        Prioridad::Alta,
        None,
        Some(&cache),
    )
    .await
    .expect("res3")
    .expect("song3");
    assert_eq!(r3.id, s1.id);

    let content_after = fs::read_to_string(&log_path).unwrap_or_default();
    let lines_after: Vec<&str> = content_after.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines_after.len(),
        1,
        "Tercera búsqueda debe salir de la caché sin nuevo proceso"
    );

    std::env::remove_var("FAKE_YTDLP_LOG");
    std::env::remove_var("FAKE_YTDLP_LOG_MATCH");
}

#[tokio::test]
async fn test_stream_info_y_prefetch() {
    let fake_bin = env!("CARGO_BIN_EXE_fake-ytdlp");
    let limiter = Limitador::new(2);
    let settings = YtDlpSettings::default();
    let cache = FuentesCache::new();

    let info = stream_info(
        fake_bin,
        "https://youtu.be/abcdefghijk",
        None,
        &settings,
        &[],
        &limiter,
        Prioridad::Alta,
        &cache,
    )
    .await
    .expect("stream_info ok");

    assert_eq!(info["id"], "abcdefghijk");
    assert!(cache.infos.has("direct|https://youtu.be/abcdefghijk"));

    // Prefetch song
    let song = Cancion {
        fuente: "youtube".to_string(),
        id: "abcdefghijk".to_string(),
        nombre: "Echo".to_string(),
        artista: "".to_string(),
        url: "https://youtu.be/abcdefghijk".to_string(),
        stream_url: None,
        es_directo: false,
        duracion: 60,
        miniatura: None,
        autor_url: None,
        edad_restringida: false,
        es_letra: false,
    };

    let prefetched = prefetch(fake_bin, &song, None, &settings, &[], &limiter, &cache)
        .await
        .expect("prefetch ok");
    assert_eq!(prefetched["id"], "abcdefghijk");

    // Prefetch query
    let q_song = prefetch_query(
        fake_bin,
        "query prefetch",
        None,
        &settings,
        &[],
        &limiter,
        &cache,
    )
    .await
    .expect("prefetch_query ok");
    assert!(q_song.is_some());
}
