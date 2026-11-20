use medios::Cancion;
use player::{ElementoCola, GestorColas, ModoRepeticion};
use std::sync::Arc;
use tokio::sync::Barrier;

fn test_cancion(n: u64) -> Cancion {
    Cancion {
        fuente: "youtube".to_string(),
        id: format!("vid{n}"),
        nombre: format!("Song {n}"),
        artista: format!("Artist {n}"),
        url: format!("https://www.youtube.com/watch?v=vid{n}"),
        stream_url: Some("https://audio.example/track.opus".to_string()),
        es_directo: false,
        duracion: 100 + n,
        miniatura: None,
        autor_url: None,
        edad_restringida: false,
        es_letra: false,
    }
}

#[tokio::test]
async fn test_modo_repeticion_ciclo_y_parseo() {
    let modo = ModoRepeticion::Apagado;
    assert_eq!(modo.siguiente(), ModoRepeticion::Cancion);
    assert_eq!(modo.siguiente().siguiente(), ModoRepeticion::Cola);
    assert_eq!(
        modo.siguiente().siguiente().siguiente(),
        ModoRepeticion::Apagado
    );

    assert_eq!(ModoRepeticion::desde_u8(0), ModoRepeticion::Apagado);
    assert_eq!(ModoRepeticion::desde_u8(1), ModoRepeticion::Cancion);
    assert_eq!(ModoRepeticion::desde_u8(2), ModoRepeticion::Cola);
    assert_eq!(ModoRepeticion::desde_u8(3), ModoRepeticion::Apagado);

    assert_eq!(
        ModoRepeticion::desde_str("off"),
        Some(ModoRepeticion::Apagado)
    );
    assert_eq!(
        ModoRepeticion::desde_str("track"),
        Some(ModoRepeticion::Cancion)
    );
    assert_eq!(
        ModoRepeticion::desde_str("queue"),
        Some(ModoRepeticion::Cola)
    );
    assert_eq!(ModoRepeticion::desde_str("invalido"), None);
}

#[tokio::test]
async fn test_gestor_colas_bloqueo_serializa_concurrencia() {
    let gestor = GestorColas::nuevo();
    let guild_id = "111111111111111111";

    let cola_arc = gestor.obtener_cola(guild_id).await;
    let barrier = Arc::new(Barrier::new(3));

    let mut handles = Vec::new();
    for i in 1..=2 {
        let cola_ref = cola_arc.clone();
        let b = barrier.clone();
        handles.push(tokio::spawn(async move {
            b.wait().await;
            let mut lock = cola_ref.lock().await;
            let song = test_cancion(i);
            lock.canciones.push(ElementoCola::nuevo(song, None));
        }));
    }

    barrier.wait().await;
    for h in handles {
        h.await.unwrap();
    }

    let final_cola = cola_arc.lock().await;
    assert_eq!(final_cola.canciones.len(), 2);
    let ids: Vec<&str> = final_cola
        .canciones
        .iter()
        .map(|s| s.cancion.id.as_str())
        .collect();
    assert!(ids.contains(&"vid1"));
    assert!(ids.contains(&"vid2"));
}

#[tokio::test]
async fn test_gestor_deshacer_expiracion_y_limpieza() {
    let gestor = GestorColas::nuevo();
    let guild_id = "guild-123";

    let songs = vec![ElementoCola::nuevo(test_cancion(1), None)];
    gestor.guardar_deshacer(guild_id, songs, 1000).await;

    // Antes de expirar
    gestor.limpiar_deshacer_expirados(500).await;
    let entrada = gestor.obtener_deshacer(guild_id).await;
    assert!(entrada.is_some());

    // Guardar de nuevo y limpiar tras expiración
    let songs2 = vec![ElementoCola::nuevo(test_cancion(2), None)];
    gestor.guardar_deshacer(guild_id, songs2, 1000).await;
    gestor.limpiar_deshacer_expirados(1500).await;

    let entrada_expirada = gestor.obtener_deshacer(guild_id).await;
    assert!(entrada_expirada.is_none());
}
