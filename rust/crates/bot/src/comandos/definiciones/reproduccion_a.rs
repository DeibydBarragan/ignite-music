//! Comandos de reproducción lote A (Spec S-22).

use super::{comando_base, opcion_boolean, opcion_string, CuerpoComando, EleccionOpcion};
use nucleo::Idiomas;
use std::collections::BTreeMap;

fn opciones_filtro() -> Vec<EleccionOpcion> {
    vec![
        EleccionOpcion {
            name: "None".into(),
            value: "none".into(),
        },
        EleccionOpcion {
            name: "Bass Boost".into(),
            value: "bassboost".into(),
        },
        EleccionOpcion {
            name: "Nightcore".into(),
            value: "nightcore".into(),
        },
        EleccionOpcion {
            name: "Vaporwave".into(),
            value: "vaporwave".into(),
        },
        EleccionOpcion {
            name: "8D".into(),
            value: "3d".into(),
        },
        EleccionOpcion {
            name: "Karaoke".into(),
            value: "karaoke".into(),
        },
        EleccionOpcion {
            name: "Echo".into(),
            value: "echo".into(),
        },
        EleccionOpcion {
            name: "Surround".into(),
            value: "surround".into(),
        },
        EleccionOpcion {
            name: "Tremolo".into(),
            value: "tremolo".into(),
        },
    ]
}

fn opciones_bucle() -> Vec<EleccionOpcion> {
    vec![
        EleccionOpcion {
            name: "Off".into(),
            value: "off".into(),
        },
        EleccionOpcion {
            name: "Song".into(),
            value: "song".into(),
        },
        EleccionOpcion {
            name: "Queue".into(),
            value: "queue".into(),
        },
    ]
}

pub(crate) fn btree_locs(idiomas: &Idiomas, key: &str) -> Option<BTreeMap<String, String>> {
    let raw = idiomas.localizations(key);
    if raw.is_empty() {
        None
    } else {
        let mut map = BTreeMap::new();
        for (k, v) in raw {
            map.insert(k, v);
        }
        Some(map)
    }
}

pub fn definiciones(idiomas: &Idiomas) -> Vec<CuerpoComando> {
    let mut cmds = Vec::with_capacity(8);

    // 1. autoplay
    cmds.push(comando_base(
        "autoplay",
        "Turn autoplay of related songs on or off",
        idiomas,
    ));

    // 2. filter
    let mut filter = comando_base("filter", "Apply an audio filter", idiomas);
    filter.options.push(opcion_string(
        "name",
        "Filter",
        true,
        None,
        None,
        Some(opciones_filtro()),
        None,
    ));
    cmds.push(filter);

    // 3. loop
    let mut loop_cmd = comando_base(
        "loop",
        "Repeat the current song or the whole queue",
        idiomas,
    );
    loop_cmd.options.push(opcion_string(
        "mode",
        "Loop mode",
        true,
        None,
        None,
        Some(opciones_bucle()),
        None,
    ));
    cmds.push(loop_cmd);

    // 4. lyrics
    let mut lyrics = comando_base(
        "lyrics",
        "Show the lyrics of the current song or any song",
        idiomas,
    );
    lyrics.options.push(opcion_string(
        "song",
        "Song to look up (defaults to the current song)",
        false,
        Some(200),
        None,
        None,
        None,
    ));
    cmds.push(lyrics);

    // 5. new-playlist
    let mut np = comando_base(
        "new-playlist",
        "Importar una playlist por enlace y guardarla en tu colección",
        idiomas,
    );
    np.options.push(opcion_string(
        "url",
        "Enlace de Spotify o YouTube",
        true,
        Some(1000),
        None,
        None,
        None,
    ));
    np.options.push(opcion_string(
        "nombre",
        "Nombre personalizado para la playlist",
        false,
        Some(60),
        None,
        None,
        None,
    ));
    cmds.push(np);

    // 6. nowplaying
    cmds.push(comando_base(
        "nowplaying",
        "Show the player panel again at the bottom of the chat",
        idiomas,
    ));

    // 7. pause
    cmds.push(comando_base("pause", "Pause the music", idiomas));

    // 8. play
    let mut play = comando_base(
        "play",
        "Play a song or playlist from YouTube, Spotify or a link",
        idiomas,
    );
    let play_locs = btree_locs(idiomas, "commands.play.query_description");
    play.options.push(opcion_string(
        "query",
        "Song name or link",
        true,
        Some(1000),
        Some(true),
        None,
        play_locs,
    ));
    play.options.push(opcion_boolean(
        "next",
        "Play it right after the current song",
        false,
    ));
    cmds.push(play);

    cmds
}
