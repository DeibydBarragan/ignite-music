use medios::Cancion;
use player::{
    al_iniciar_cancion, comprobar_corte_prematuro, encolar, Cola, ElementoCola, ErrorReproducir,
    ModoRepeticion,
};

fn make_song(n: u64, age_restricted: bool) -> ElementoCola {
    let cancion = Cancion {
        fuente: "youtube".to_string(),
        id: format!("vid{n}"),
        nombre: format!("Song {n}"),
        artista: format!("Artist {n}"),
        url: format!("https://www.youtube.com/watch?v=vid{n}"),
        stream_url: Some("https://audio.example/track.opus".to_string()),
        es_directo: false,
        duracion: 200,
        miniatura: None,
        autor_url: None,
        edad_restringida: age_restricted,
        es_letra: false,
    };
    ElementoCola::nuevo(cancion, None)
}

#[test]
fn spotify_se_reproduce_con_busqueda_y_conserva_su_identidad() {
    let mut cola = Cola::nueva("guild-1");
    let mut track = make_song(1, false);
    track.cancion.fuente = "spotify".into();
    track.cancion.url = "https://open.spotify.com/track/example".into();
    track.cancion.nombre = "Canción".into();
    track.cancion.artista = "Artista".into();
    encolar(&mut cola, vec![track], 0, false, 5, true).unwrap();
    assert_eq!(
        player::reproduccion::primera_cancion_url(&cola).as_deref(),
        Some("ytsearch1:Artista - Canción")
    );
    assert_eq!(
        cola.canciones[0].cancion.url,
        "https://open.spotify.com/track/example"
    );
    assert_eq!(cola.canciones[0].cancion.fuente, "spotify");
    cola.canciones[0].cancion.artista.clear();
    assert_eq!(
        player::reproduccion::primera_cancion_url(&cola).as_deref(),
        Some("ytsearch1:Canción")
    );
}

#[test]
fn test_encolar_capacidad_y_truncado() {
    let mut cola = Cola::nueva("guild-1");
    let initial = vec![make_song(1, false), make_song(2, false)];
    let outcome1 = encolar(&mut cola, initial, 0, false, 5, true).expect("encolar");
    assert_eq!(outcome1.posicion, 0);
    assert!(!outcome1.truncado);
    assert!(outcome1.empezada);

    // Enqueue 5 songs when remaining capacity is 3 (max 5)
    let extra = vec![
        make_song(3, false),
        make_song(4, false),
        make_song(5, false),
        make_song(6, false),
        make_song(7, false),
    ];
    let outcome2 = encolar(&mut cola, extra, 0, false, 5, true).expect("encolar");
    assert!(outcome2.truncado);
    assert_eq!(cola.canciones.len(), 5);

    // Now queue is full (5/5)
    let more = vec![make_song(8, false)];
    assert_eq!(
        encolar(&mut cola, more, 0, false, 5, true),
        Err(ErrorReproducir::ColaLlena)
    );
}

#[test]
fn test_encolar_edad_restringida_nsfw() {
    let mut cola = Cola::nueva("guild-1");

    // Single age-restricted song in non-NSFW channel fails
    let restricted = vec![make_song(1, true)];
    assert_eq!(
        encolar(&mut cola, restricted, 0, false, 50, false),
        Err(ErrorReproducir::NoPermitidoNonNsfw)
    );

    // List with all restricted songs fails with EMPTY_FILTERED_PLAYLIST
    let all_restricted = vec![make_song(1, true), make_song(2, true)];
    assert_eq!(
        encolar(&mut cola, all_restricted, 0, false, 50, false),
        Err(ErrorReproducir::ListaFiltradaVacia)
    );

    // List with mixed songs filters out restricted songs
    let mixed = vec![make_song(1, true), make_song(2, false)];
    let outcome = encolar(&mut cola, mixed, 0, false, 50, false).expect("encolar");
    assert_eq!(outcome.posicion, 0);
    assert_eq!(cola.canciones.len(), 1);
    assert_eq!(cola.canciones[0].cancion.id, "vid2");
}

#[test]
fn test_encolar_con_saltar_y_posicion() {
    let mut cola = Cola::nueva("guild-1");
    let _ = encolar(&mut cola, vec![make_song(1, false)], 0, false, 50, true);

    // Play next (posicion == 1)
    let _ = encolar(&mut cola, vec![make_song(2, false)], 1, false, 50, true);
    assert_eq!(cola.canciones.len(), 2);
    assert_eq!(cola.canciones[1].cancion.id, "vid2");

    // Play with skip: skips song 1, starts song 3
    let res = encolar(&mut cola, vec![make_song(3, false)], 0, true, 50, true).expect("skip");
    assert!(res.empezada);
    assert_eq!(cola.canciones[0].cancion.id, "vid3");
    assert_eq!(cola.canciones_previas.len(), 1);
    assert_eq!(cola.canciones_previas[0].cancion.id, "vid1");
}

#[test]
fn test_auto_recuperacion_corte_prematuro() {
    let mut cola = Cola::nueva("guild-1");
    let mut song = make_song(1, false);
    song.cancion.duracion = 200; // 3m20s
    cola.canciones.push(song);
    cola.modo_repeticion = ModoRepeticion::Apagado;

    // Cut off at 35s (> 5s, < 188s)
    let rec = comprobar_corte_prematuro(&mut cola, 35).expect("recuperacion");
    assert_eq!(rec.seek_target, 33);
    assert_eq!(rec.segundos_reproducidos, 35);
    assert_eq!(cola.modo_repeticion, ModoRepeticion::Cancion);
    assert_eq!(cola.canciones[0].cut_retries, 1);
    assert_eq!(
        cola.canciones[0].original_repeat_mode,
        Some(ModoRepeticion::Apagado)
    );

    // Next play event restores original repeat mode
    al_iniciar_cancion(&mut cola);
    assert_eq!(cola.modo_repeticion, ModoRepeticion::Apagado);
    assert_eq!(cola.canciones[0].original_repeat_mode, None);

    // Second cut does not retry again (exceeded max 1 retry)
    let rec2 = comprobar_corte_prematuro(&mut cola, 40);
    assert!(rec2.is_none());
}

#[test]
fn test_corte_normal_cerca_del_final_no_recupera() {
    let mut cola = Cola::nueva("guild-1");
    let mut song = make_song(1, false);
    song.cancion.duracion = 200;
    cola.canciones.push(song);

    // Finished naturally within 12s of end (195 >= 188)
    let rec = comprobar_corte_prematuro(&mut cola, 195);
    assert!(rec.is_none());
    assert_eq!(cola.modo_repeticion, ModoRepeticion::Apagado);
    assert_eq!(cola.canciones[0].cut_retries, 0);
}
