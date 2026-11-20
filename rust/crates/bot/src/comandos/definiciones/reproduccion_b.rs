//! Comandos de reproducción lote B (Spec S-22).

use super::reproduccion_a::btree_locs;
use super::{comando_base, opcion_integer, opcion_string, CuerpoComando};
use nucleo::Idiomas;

pub fn definiciones(idiomas: &Idiomas, max_volume: u32) -> Vec<CuerpoComando> {
    let mut cmds = Vec::with_capacity(9);

    // 9. playlists
    let mut pl = comando_base(
        "playlists",
        "Ver tus playlists guardadas y reproducirlas",
        idiomas,
    );
    pl.options.push(opcion_string(
        "nombre",
        "Filtrar por nombre de playlist",
        false,
        Some(60),
        None,
        None,
        None,
    ));
    pl.options.push(opcion_string(
        "reproducir",
        "Nombre o enlace de tu playlist para sonarla ahora",
        false,
        Some(500),
        None,
        None,
        None,
    ));
    pl.options.push(opcion_integer(
        "page",
        "Número de página",
        false,
        Some(1),
        None,
    ));
    cmds.push(pl);

    // 10. playnext
    let mut playnext = comando_base(
        "playnext",
        "Play a song right after the current one (jump the queue)",
        idiomas,
    );
    let playnext_locs = btree_locs(idiomas, "commands.playnext.query_description");
    playnext.options.push(opcion_string(
        "query",
        "Song name or link",
        true,
        Some(1000),
        Some(true),
        None,
        playnext_locs,
    ));
    cmds.push(playnext);

    // 11. previous
    cmds.push(comando_base(
        "previous",
        "Play the previous song again",
        idiomas,
    ));

    // 12. resume
    cmds.push(comando_base("resume", "Resume the music", idiomas));

    // 13. search
    let mut search = comando_base(
        "search",
        "Search YouTube and pick a song from the results",
        idiomas,
    );
    search.options.push(opcion_string(
        "query",
        "What to search for",
        true,
        Some(200),
        None,
        None,
        None,
    ));
    cmds.push(search);

    // 14. seek
    let mut seek = comando_base("seek", "Jump to a time in the current song", idiomas);
    seek.options.push(opcion_string(
        "time",
        "For example 1:30 or 90",
        true,
        Some(10),
        None,
        None,
        None,
    ));
    cmds.push(seek);

    // 15. skip
    cmds.push(comando_base("skip", "Skip the current song", idiomas));

    // 16. stop
    cmds.push(comando_base(
        "stop",
        "Stop the music, clear the queue and leave the voice channel",
        idiomas,
    ));

    // 17. volume
    let mut volume = comando_base("volume", "Change the volume", idiomas);
    let vol_desc = format!("Volume from 0 to {}", max_volume);
    volume.options.push(opcion_integer(
        "level",
        &vol_desc,
        true,
        Some(0),
        Some(max_volume as i64),
    ));
    cmds.push(volume);

    cmds
}
