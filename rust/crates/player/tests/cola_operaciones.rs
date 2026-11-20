use medios::Cancion;
use player::{
    deshacer, limpiar, mover, ordenar, quitar_por_indice, quitar_por_track_id, saltar_a, Cola,
    ElementoCola, ErrorOperacionCola, GestorColas, UNDO_TTL_MS,
};

fn fake_song(n: u64, track_id: Option<&str>) -> ElementoCola {
    let cancion = Cancion {
        fuente: "youtube".to_string(),
        id: format!("vid{n}"),
        nombre: format!("Song {n}"),
        artista: format!("Artist {n}"),
        url: format!("https://www.youtube.com/watch?v=vid{n}"),
        stream_url: Some("https://example.com/stream.opus".to_string()),
        es_directo: false,
        duracion: 100 + n,
        miniatura: None,
        autor_url: None,
        edad_restringida: false,
        es_letra: false,
    };
    let mut elem = ElementoCola::nuevo(cancion, None);
    if let Some(tid) = track_id {
        elem.track_id = tid.to_string();
    }
    elem
}

#[tokio::test]
async fn test_quitar_por_indice_y_limites() {
    let mut cola = Cola::nueva("guild-1");
    cola.canciones.push(fake_song(1, None));
    cola.canciones.push(fake_song(2, None));
    cola.canciones.push(fake_song(3, None));

    // Refuses current song (index 0) and out-of-range indexes
    assert_eq!(
        quitar_por_indice(&mut cola, 0),
        Err(ErrorOperacionCola::PosicionInvalida)
    );
    assert_eq!(
        quitar_por_indice(&mut cola, 9),
        Err(ErrorOperacionCola::PosicionInvalida)
    );

    // Removes upcoming song at index 2 (Song 3)
    let removed = quitar_por_indice(&mut cola, 2).expect("quitar");
    assert_eq!(removed.cancion.nombre, "Song 3");
    assert_eq!(cola.canciones.len(), 2);
    assert_eq!(cola.canciones[0].cancion.nombre, "Song 1");
    assert_eq!(cola.canciones[1].cancion.nombre, "Song 2");
}

#[tokio::test]
async fn test_quitar_por_track_id_nunca_quita_la_actual() {
    let mut cola = Cola::nueva("guild-1");
    cola.canciones.push(fake_song(1, Some("current-track-1")));
    cola.canciones.push(fake_song(2, Some("kZ9_x-12Ab")));

    // Removes upcoming
    let removed = quitar_por_track_id(&mut cola, "kZ9_x-12Ab").expect("quitar");
    assert_eq!(removed.cancion.id, "vid2");
    assert_eq!(cola.canciones.len(), 1);

    // Never removes current song (even with matching track_id)
    assert_eq!(
        quitar_por_track_id(&mut cola, "current-track-1"),
        Err(ErrorOperacionCola::CancionNoEncontrada)
    );
    // Non-existent trackId
    assert_eq!(
        quitar_por_track_id(&mut cola, "nope-nope-nope"),
        Err(ErrorOperacionCola::CancionNoEncontrada)
    );
}

#[tokio::test]
async fn test_ordenar_y_mover() {
    let mut cola = Cola::nueva("guild-1");
    cola.canciones.push(fake_song(1, None));
    cola.canciones.push(fake_song(2, Some("track-a")));
    cola.canciones.push(fake_song(3, Some("track-b")));
    cola.canciones.push(fake_song(4, Some("track-c")));

    // Move track-c (pos 3) to pos 1
    let title = ordenar(&mut cola, "track-c", 1).expect("ordenar");
    assert_eq!(title, "Song 4");

    let ids: Vec<&str> = cola
        .canciones
        .iter()
        .map(|s| s.cancion.id.as_str())
        .collect();
    assert_eq!(ids, vec!["vid1", "vid4", "vid2", "vid3"]);

    // Invalid move out-of-range or index 0
    assert_eq!(
        mover(&mut cola, 0, 1),
        Err(ErrorOperacionCola::PosicionInvalida)
    );
    assert_eq!(
        mover(&mut cola, 1, 99),
        Err(ErrorOperacionCola::PosicionInvalida)
    );
}

#[tokio::test]
async fn test_saltar_a_posicion() {
    let mut cola = Cola::nueva("guild-1");
    cola.canciones.push(fake_song(1, None));
    cola.canciones.push(fake_song(2, None));
    cola.canciones.push(fake_song(3, None));
    cola.pausada = true;

    // Invalid position
    assert_eq!(
        saltar_a(&mut cola, 0),
        Err(ErrorOperacionCola::PosicionInvalida)
    );
    assert_eq!(
        saltar_a(&mut cola, 5),
        Err(ErrorOperacionCola::PosicionInvalida)
    );

    // Jump to index 2 (Song 3)
    let nueva_actual = saltar_a(&mut cola, 2).expect("saltar_a");
    assert_eq!(nueva_actual.cancion.id, "vid3");
    assert_eq!(cola.canciones.len(), 1);
    assert_eq!(cola.canciones_previas.len(), 2);
    assert!(!cola.pausada);
}

#[tokio::test]
async fn test_limpiar_y_deshacer_con_ttl_60s() {
    let gestor = GestorColas::nuevo();
    let mut cola = Cola::nueva("guild-1");
    cola.canciones.push(fake_song(1, None));
    cola.canciones.push(fake_song(2, None));
    cola.canciones.push(fake_song(3, None));

    let ahora_ms = 1_000_000;
    let cleared = limpiar(&mut cola, ahora_ms, &gestor)
        .await
        .expect("limpiar");
    assert_eq!(cleared, 2);
    assert_eq!(cola.canciones.len(), 1);
    assert_eq!(cola.canciones[0].cancion.id, "vid1");

    // Deshacer dentro del TTL de 60s
    let restored = deshacer(&mut cola, ahora_ms + 10_000, &gestor)
        .await
        .expect("deshacer");
    assert_eq!(restored, 2);
    assert_eq!(cola.canciones.len(), 3);
    assert_eq!(cola.canciones[1].cancion.id, "vid2");
    assert_eq!(cola.canciones[2].cancion.id, "vid3");

    // Segundo deshacer falla con UNDO_EMPTY
    assert_eq!(
        deshacer(&mut cola, ahora_ms + 10_000, &gestor).await,
        Err(ErrorOperacionCola::DeshacerVacio)
    );

    // Limpiar de nuevo y probar expiración pasados 60s
    let _ = limpiar(&mut cola, ahora_ms, &gestor)
        .await
        .expect("limpiar");
    let res_expirado = deshacer(&mut cola, ahora_ms + UNDO_TTL_MS + 1, &gestor).await;
    assert_eq!(res_expirado, Err(ErrorOperacionCola::DeshacerExpirado));
    assert_eq!(cola.canciones.len(), 1);
}
