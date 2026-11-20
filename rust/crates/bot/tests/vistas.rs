use bot::ui::ayuda::{vista_ayuda, EnlacesAyuda, InfoComandoAyuda, StatsAyuda};
use bot::ui::busqueda::vista_busqueda;
use bot::ui::cola::{vista_cola, DatosItemCola};
use bot::ui::idioma::vista_idioma;
use bot::ui::letras::vista_letras;
use bot::ui::listas::{vista_playlists, ItemPlaylistUsuario};
use bot::ui::panel::vista_terminada;
use medios::Cancion;
use nucleo::idioma::Idiomas;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

fn fake_song(n: u64, name: Option<String>, dur: u64) -> Cancion {
    Cancion {
        fuente: "youtube".into(),
        id: format!("vid{:08}", n),
        nombre: name.unwrap_or_else(|| format!("Song {n}")),
        artista: format!("Artist {n}"),
        url: format!("https://www.youtube.com/watch?v=vid{:08}", n),
        stream_url: None,
        es_directo: false,
        duracion: dur,
        miniatura: Some(format!("https://i.ytimg.com/vi/vid{n}/hqdefault.jpg")),
        autor_url: None,
        edad_restringida: false,
        es_letra: false,
    }
}

fn cargar_idiomas() -> Arc<Idiomas> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../languages");
    Arc::new(Idiomas::cargar(&path, Some("en")).unwrap())
}

fn normalize(val: &mut serde_json::Value) {
    match val {
        serde_json::Value::Object(map) => {
            map.retain(|k, v| {
                !v.is_null()
                    && !(k == "disabled" && v == &serde_json::Value::Bool(false))
                    && !(k == "default" && v == &serde_json::Value::Bool(false))
            });
            for v in map.values_mut() {
                normalize(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                normalize(v);
            }
        }
        _ => {}
    }
}

#[test]
fn test_paridad_vistas_con_node() {
    let idiomas = cargar_idiomas();
    let fix_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vistas-node.json");
    let json_str = std::fs::read_to_string(&fix_path).expect("vistas-node.json exists");
    let fixture: HashMap<String, serde_json::Value> = serde_json::from_str(&json_str).unwrap();

    let songs45: Vec<DatosItemCola> = (1..=45)
        .map(|i| {
            let s = fake_song(i, None, 180 + i);
            let (req_id, bot) = if i == 1 {
                (Some("111111111111111111".to_string()), false)
            } else if i == 2 {
                (Some("222222222222222222".to_string()), true)
            } else {
                (None, false)
            };
            DatosItemCola {
                cancion: s,
                requester_id: req_id,
                requester_es_bot: bot,
            }
        })
        .collect();

    let cur = &songs45[0];
    let upcoming = &songs45[1..];

    let lyrics_long = format!(
        "{}\n\n{}",
        (0..200)
            .map(|i| format!("Line {i} [chorus] *x*"))
            .collect::<Vec<_>>()
            .join("\n"),
        "z".repeat(7000)
    );

    let search_results: Vec<Cancion> = (1..=10)
        .map(|i| fake_song(i, Some(format!("Search Result {i}")), 180 + i))
        .collect();

    let help_commands: Vec<InfoComandoAyuda> = (1..=22)
        .map(|i| InfoComandoAyuda {
            name: format!("cmd{i}"),
            id: Some(format!("12345678901234567{}", i - 1)),
            description: format!("Description for cmd{i}"),
        })
        .collect();

    let help_stats = StatsAyuda {
        servers: 10,
        active: 3,
        uptime: "1d 2h 30m".to_string(),
        ping: 42,
    };
    let help_enlaces = EnlacesAyuda {
        support_server: Some("https://discord.gg/ignite".to_string()),
        website: Some("https://ignite.bot".to_string()),
        invite: Some(
            "https://discord.com/oauth2/authorize?client_id=1557938456225915001&permissions=3230720&scope=bot%20applications.commands".to_string(),
        ),
    };

    let playlists15: Vec<ItemPlaylistUsuario> = (1..=15)
        .map(|i| ItemPlaylistUsuario {
            name: format!("Playlist {i}"),
            url: format!("https://youtube.com/playlist?list=PL{i}"),
            source: if i % 2 == 1 { "youtube" } else { "spotify" }.to_string(),
            track_count: (i + 1) as usize,
            duration: ((i + 1) * 180) as u64,
        })
        .collect();

    let mut casos = HashMap::new();
    casos.insert(
        "queue_page_0",
        vista_cola(&idiomas, "en", Some(cur), upcoming, 0),
    );
    casos.insert(
        "queue_page_2",
        vista_cola(&idiomas, "en", Some(cur), upcoming, 2),
    );
    casos.insert(
        "queue_page_last",
        vista_cola(&idiomas, "en", Some(cur), upcoming, 99),
    );
    casos.insert("queue_empty", vista_cola(&idiomas, "en", Some(cur), &[], 0));
    casos.insert(
        "lyrics_paginated",
        vista_letras(
            &idiomas,
            "en",
            "Song Title",
            Some("Artist Name"),
            &lyrics_long,
            Some("LRCLIB"),
            Some("deadbeef1234"),
            1,
        ),
    );
    casos.insert(
        "lyrics_short",
        vista_letras(
            &idiomas,
            "en",
            "Short Song",
            None,
            "Line 1\nLine 2",
            Some("LRCLIB"),
            None,
            0,
        ),
    );
    casos.insert(
        "search",
        vista_busqueda(
            &idiomas,
            "en",
            "rock music",
            &search_results,
            "deadbeef1234",
        ),
    );
    casos.insert(
        "help",
        vista_ayuda(
            &idiomas,
            "en",
            &help_commands,
            Some(&help_stats),
            Some(&help_enlaces),
        ),
    );
    casos.insert(
        "language",
        vista_idioma(&idiomas, "en", &idiomas.list(), "es"),
    );
    casos.insert(
        "playlists_with_items",
        vista_playlists(&idiomas, "en", &playlists15, 15, 0, Some("rock")),
    );
    casos.insert(
        "playlists_empty",
        vista_playlists(&idiomas, "en", &[], 0, 0, None),
    );
    let s1 = fake_song(1, None, 181);
    casos.insert(
        "ended_panel",
        vista_terminada(&idiomas, "en", "Done", Some("Playback finished"), Some(&s1)),
    );

    for (k, rust_payload) in casos {
        let exp = &fixture[k];
        let rust_flags = rust_payload.flags.unwrap().bits();
        assert_eq!(
            rust_flags,
            exp["flags"].as_u64().unwrap(),
            "flags mismatch in {k}"
        );
        let mut rust_comp = serde_json::to_value(&rust_payload.components).unwrap();
        let mut exp_comp = exp["components"].clone();
        normalize(&mut rust_comp);
        normalize(&mut exp_comp);
        assert_eq!(rust_comp, exp_comp, "components mismatch in {k}");
    }
}
