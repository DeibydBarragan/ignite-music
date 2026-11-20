use medios::letras::{
    dividir_letras, elegir_mejor, limpiar_titulo, CacheLetras, ItemLetraLrc, ResultadoLetras,
};
use std::time::Duration;

#[test]
fn test_limpiar_titulo_paridad_con_node() {
    let (t1, a1) = limpiar_titulo(
        "Rick Astley - Never Gonna Give You Up (Official Music Video)",
        "RickAstleyVEVO",
    );
    assert_eq!(t1, "Never Gonna Give You Up");
    assert_eq!(a1, "Rick Astley");

    let (t2, a2) = limpiar_titulo("Song [Lyrics] ft. Someone", "Band - Topic");
    assert_eq!(t2, "Song");
    assert_eq!(a2, "Band");

    let (t3, _) = limpiar_titulo("(Official Video)", "");
    assert_eq!(t3, "(Official Video)");
}

#[test]
fn test_elegir_mejor_paridad_con_node() {
    let results = vec![
        ItemLetraLrc {
            track_name: Some("Song A".into()),
            artist_name: Some("Artist".into()),
            plain_lyrics: Some("a".into()),
            instrumental: false,
            duration: 300.0,
        },
        ItemLetraLrc {
            track_name: Some("Song B".into()),
            artist_name: Some("Artist".into()),
            plain_lyrics: Some("b".into()),
            instrumental: false,
            duration: 212.0,
        },
        ItemLetraLrc {
            track_name: Some("Song C".into()),
            artist_name: Some("Artist".into()),
            plain_lyrics: None,
            instrumental: false,
            duration: 213.0,
        },
        ItemLetraLrc {
            track_name: Some("Song D".into()),
            artist_name: Some("Artist".into()),
            plain_lyrics: Some("c".into()),
            instrumental: true,
            duration: 213.0,
        },
    ];

    let best1 = elegir_mejor(&results, 213).unwrap();
    assert_eq!(best1.plain_lyrics.as_deref(), Some("b"));

    let best2 = elegir_mejor(&results, 0).unwrap();
    assert_eq!(best2.plain_lyrics.as_deref(), Some("a"));

    assert!(elegir_mejor(&[], 1).is_none());
}

#[test]
fn test_dividir_letras_paginado() {
    let mut long_text = (0..200)
        .map(|i| format!("Line {i} [chorus] *x*"))
        .collect::<Vec<_>>()
        .join("\n");
    long_text.push_str("\n\n");
    long_text.push_str(&"z".repeat(7000));

    let pages = dividir_letras(&long_text);
    assert!(pages.len() > 2);
    for page in &pages {
        assert!(page.chars().count() <= 2800);
    }

    let empty = dividir_letras("");
    assert_eq!(empty, vec![""]);

    let short = dividir_letras("Hello world");
    assert_eq!(short, vec!["Hello world"]);
}

#[tokio::test]
async fn test_cache_letras() {
    let cache = CacheLetras::nuevo(Duration::from_millis(50), 2);
    assert!(cache.obtener("song|artist").await.is_none());

    let res = ResultadoLetras {
        title: "Song".into(),
        artist: "Artist".into(),
        lyrics: "La la la".into(),
        source: "LRCLIB".into(),
    };
    cache
        .insertar("song|artist".into(), Some(res.clone()))
        .await;

    let hit = cache.obtener("song|artist").await;
    assert_eq!(hit, Some(Some(res)));

    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(cache.obtener("song|artist").await.is_none());
}
