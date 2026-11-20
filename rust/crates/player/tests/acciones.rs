use medios::Cancion;
use player::{
    alternar_pausa, anterior, aplicar_control, buscar, fijar_bucle, fijar_filtro, fijar_volumen,
    mezclar, pausar, reanudar, saltar, Cola, ElementoCola, ModoRepeticion,
};

fn fake_queue(count: usize) -> Cola {
    let mut q = Cola::nueva("111111111111111111");
    for i in 0..count {
        let cancion = Cancion {
            fuente: "youtube".to_string(),
            id: format!("vid{i}"),
            nombre: format!("Song {i}"),
            artista: format!("Artist {i}"),
            url: format!("https://www.youtube.com/watch?v=vid{i}"),
            stream_url: Some("https://example.com/audio.opus".to_string()),
            es_directo: false,
            duracion: 100 + i as u64,
            miniatura: None,
            autor_url: None,
            edad_restringida: false,
            es_letra: false,
        };
        q.canciones.push(ElementoCola::nuevo(cancion, None));
    }
    q
}

#[test]
fn test_pausa_reanudar_y_alternar() {
    let mut q = fake_queue(3);
    assert!(!reanudar(&mut q).ok);
    assert!(pausar(&mut q).ok);
    assert!(!pausar(&mut q).ok);
    alternar_pausa(&mut q);
    assert!(!q.pausada);
}

#[test]
fn test_saltar_y_anterior() {
    let mut lone = fake_queue(1);
    assert!(!saltar(&mut lone).ok);
    lone.autoplay = true;
    assert!(saltar(&mut lone).ok);

    let mut q = fake_queue(3);
    assert!(!anterior(&mut q).ok);
    let mut song = fake_queue(1).canciones.pop().unwrap();
    song.cancion.nombre = "Prev Song".to_string();
    q.canciones_previas.push(song);
    assert!(anterior(&mut q).ok);
    assert_eq!(q.canciones[0].cancion.nombre, "Prev Song");
}

#[test]
fn test_fijar_bucle_ciclo_y_explicito() {
    let mut q = fake_queue(3);
    fijar_bucle(&mut q, None);
    assert_eq!(q.modo_repeticion, ModoRepeticion::Cancion);
    fijar_bucle(&mut q, None);
    assert_eq!(q.modo_repeticion, ModoRepeticion::Cola);
    fijar_bucle(&mut q, None);
    assert_eq!(q.modo_repeticion, ModoRepeticion::Apagado);

    let res = fijar_bucle(&mut q, Some(ModoRepeticion::Cola));
    assert_eq!(res.clave, "buttonhandler.loop_mode_queue");
    assert_eq!(q.modo_repeticion, ModoRepeticion::Cola);
}

#[test]
fn test_validacion_volumen() {
    let mut q = fake_queue(3);
    assert!(!fijar_volumen(&mut q, 150, 100).ok);
    assert!(!fijar_volumen(&mut q, -1, 100).ok);
    assert!(fijar_volumen(&mut q, 0, 100).ok);
    assert!(fijar_volumen(&mut q, 100, 100).ok);
    assert_eq!(q.volumen, 100);
}

#[test]
fn test_filtros_establecer_y_limpiar() {
    let mut q = fake_queue(3);
    assert!(fijar_filtro(&mut q, "bassboost").ok);
    assert_eq!(q.filtros, vec!["bassboost".to_string()]);
    assert!(fijar_filtro(&mut q, "none").ok);
    assert!(q.filtros.is_empty());
}

#[test]
fn test_buscar_tiempo_y_directos() {
    let mut q = fake_queue(3);
    assert!(!buscar(&mut q, 10_000).ok); // past duration
    assert!(buscar(&mut q, 30).ok);
    assert_eq!(q.posicion_segundos, 30);

    q.canciones[0].cancion.es_directo = true;
    assert!(!buscar(&mut q, 30).ok);
}

#[test]
fn test_mezclar_minimo_canciones() {
    let mut q2 = fake_queue(2);
    assert!(!mezclar(&mut q2).ok);

    let mut q3 = fake_queue(3);
    let first_id = q3.canciones[0].cancion.id.clone();
    assert!(mezclar(&mut q3).ok);
    assert_eq!(q3.canciones[0].cancion.id, first_id);
}

#[test]
fn test_aplicar_control_despacha_todas_las_acciones() {
    let acciones = [
        ("pause", None),
        ("resume", None),
        ("toggle", None),
        ("skip", None),
        ("previous", None),
        ("shuffle", None),
        ("autoplay", None),
        ("filter", Some("bassboost")),
        ("volume", Some("75")),
        ("seek", Some("10")),
        ("loop", Some("queue")),
        ("stop", None),
    ];

    for (act, val) in acciones {
        let mut q = fake_queue(5);
        let res = aplicar_control(&mut q, act, val, 100);
        assert!(res.is_some(), "aplicar_control({act}) debe devolver Some");
    }

    let mut q = fake_queue(3);
    assert!(aplicar_control(&mut q, "desconocido", None, 100).is_none());
}
