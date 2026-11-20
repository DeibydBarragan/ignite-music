//! Pruebas unitarias del Hub SSE y paridad con Node (Spec S-28).

use axum::body::Bytes;
use bot::api::sse::{HubSse, CAPACIDAD_CANAL_SSE, DEFAULT_HEARTBEAT_MS};
use futures_util::StreamExt;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_sse_paridad_formato_node() {
    assert_eq!(DEFAULT_HEARTBEAT_MS, 25_000);
    assert_eq!(CAPACIDAD_CANAL_SSE, 64);
    let hub = HubSse::nuevo(Duration::ZERO);

    let (_id, mut stream) = hub.suscribir("123456789012345678");
    assert_eq!(hub.size(Some("123456789012345678")), 1);

    // 1. Conexión inicial emite `: connected\n\n` exactamente como Node
    let first = stream.next().await.expect("primer chunk").unwrap();
    assert_eq!(first, Bytes::from_static(b": connected\n\n"));

    // 2. Publicación emite `event: ...\ndata: ...\n\n` exactamente como Node
    let delivered = hub.publicar(
        "123456789012345678",
        "player.sync",
        &json!({ "hasQueue": true }),
    );
    assert_eq!(delivered, 1);

    let second = stream.next().await.expect("segundo chunk").unwrap();
    let expected = "event: player.sync\ndata: {\"hasQueue\":true}\n\n";
    assert_eq!(second, Bytes::from(expected));

    // 3. Latido emite `: heartbeat\n\n` exactamente como Node
    let latidos = hub.enviar_latidos();
    assert_eq!(latidos, 1);

    let third = stream.next().await.expect("tercer chunk").unwrap();
    assert_eq!(third, Bytes::from_static(b": heartbeat\n\n"));

    hub.cerrar();
}

#[tokio::test]
async fn test_sse_dos_suscriptores_mismo_servidor_y_aislamiento() {
    let hub = HubSse::nuevo(Duration::ZERO);
    let (_id1, mut stream_a1) = hub.suscribir("111111111111111111");
    let (_id2, mut stream_a2) = hub.suscribir("111111111111111111");
    let (_id3, mut stream_b) = hub.suscribir("222222222222222222");

    // Drenar connected
    assert_eq!(
        stream_a1.next().await.unwrap().unwrap(),
        Bytes::from_static(b": connected\n\n")
    );
    assert_eq!(
        stream_a2.next().await.unwrap().unwrap(),
        Bytes::from_static(b": connected\n\n")
    );
    assert_eq!(
        stream_b.next().await.unwrap().unwrap(),
        Bytes::from_static(b": connected\n\n")
    );

    // Publicar en guild 111111111111111111
    let entregados = hub.publicar(
        "111111111111111111",
        "player.sync",
        &json!({ "volume": 80 }),
    );
    assert_eq!(entregados, 2);

    let ev_a1 = stream_a1.next().await.unwrap().unwrap();
    let ev_a2 = stream_a2.next().await.unwrap().unwrap();
    assert_eq!(
        ev_a1,
        Bytes::from("event: player.sync\ndata: {\"volume\":80}\n\n")
    );
    assert_eq!(
        ev_a2,
        Bytes::from("event: player.sync\ndata: {\"volume\":80}\n\n")
    );

    // El suscriptor del servidor B no recibe nada de A
    assert_eq!(hub.size(Some("222222222222222222")), 1);
    assert_eq!(hub.size(None), 3);

    hub.cerrar();
}

#[tokio::test]
async fn test_sse_sin_suscriptores_no_calcula_estado() {
    let hub = HubSse::nuevo(Duration::ZERO);
    let contador = Arc::new(AtomicUsize::new(0));

    let c1 = Arc::clone(&contador);
    let res = hub
        .publicar_sincronizacion_con("333333333333333333", || async move {
            c1.fetch_add(1, Ordering::SeqCst);
            json!({ "state": "computed" })
        })
        .await;

    assert_eq!(res, 0);
    assert_eq!(
        contador.load(Ordering::SeqCst),
        0,
        "No debe calcular estado si no hay suscriptores"
    );

    // Ahora agregamos un suscriptor
    let (_id, mut stream) = hub.suscribir("333333333333333333");
    let _ = stream.next().await; // connected

    let c2 = Arc::clone(&contador);
    let res = hub
        .publicar_sincronizacion_con("333333333333333333", || async move {
            c2.fetch_add(1, Ordering::SeqCst);
            json!({ "state": "computed" })
        })
        .await;

    assert_eq!(res, 1);
    assert_eq!(
        contador.load(Ordering::SeqCst),
        1,
        "Debe calcular estado si hay suscriptores"
    );

    hub.cerrar();
}

#[tokio::test]
async fn test_sse_socket_cerrado_limpia_servidor() {
    let hub = HubSse::nuevo(Duration::ZERO);
    let (_id, stream) = hub.suscribir("444444444444444444");

    assert_eq!(hub.suscriptores_servidor("444444444444444444"), 1);
    assert!(hub.tiene_servidor("444444444444444444"));

    // Al cerrarse el socket (drop del stream), la suscripción se quita
    drop(stream);

    assert_eq!(hub.suscriptores_servidor("444444444444444444"), 0);
    assert!(
        !hub.tiene_servidor("444444444444444444"),
        "El servidor vacío sale del mapa"
    );

    hub.cerrar();
}

#[tokio::test]
async fn test_sse_cola_llena_desconecta_lento_y_mantiene_activo() {
    let hub = HubSse::nuevo(Duration::ZERO);
    let gid = "555555555555555555";

    // Suscriptor lento: no leerá eventos
    let (_lento_id, mut stream_lento) = hub.suscribir(gid);
    // Suscriptor activo: leerá eventos normalmente
    let (_activo_id, mut stream_activo) = hub.suscribir(gid);

    // Drenar connected en activo
    assert_eq!(
        stream_activo.next().await.unwrap().unwrap(),
        Bytes::from_static(b": connected\n\n")
    );

    // Llenamos la cola de 64 en ambos (el lento tiene ya 1 con ": connected\n\n")
    // Mandamos 63 eventos
    for i in 0..63 {
        hub.publicar(gid, "tick", &json!({ "i": i }));
        // El activo consume para hacer espacio
        let _ = stream_activo.next().await.unwrap().unwrap();
    }
    // Ahora el lento tiene 1 (connected) + 63 (ticks) = 64 (capacidad máxima de 64)
    // El próximo evento desbordará el canal del suscriptor lento
    hub.publicar(gid, "overflow", &json!({ "overflow": true }));

    // El suscriptor lento fue expulsado del mapa
    assert_eq!(hub.suscriptores_servidor(gid), 1);

    // El suscriptor activo sigue recibiendo
    let ev_activo = stream_activo.next().await.unwrap().unwrap();
    assert_eq!(
        ev_activo,
        Bytes::from("event: overflow\ndata: {\"overflow\":true}\n\n")
    );

    // El suscriptor lento puede drenar lo que tenía en cola y luego el canal se cierra (None)
    for _ in 0..64 {
        assert!(stream_lento.next().await.is_some());
    }
    // Canal cerrado al finalizar el buffer
    assert!(stream_lento.next().await.is_none());

    hub.cerrar();
}

#[tokio::test]
async fn test_sse_apagado_cierra_todos_los_streams() {
    let hub = HubSse::nuevo(Duration::ZERO);
    let (_id1, mut stream1) = hub.suscribir("666666666666666666");
    let (_id2, mut stream2) = hub.suscribir("777777777777777777");

    // Drenar connected
    let _ = stream1.next().await;
    let _ = stream2.next().await;

    // Apagar el hub
    hub.cerrar();

    // Todos los streams terminan inmediatamente
    assert!(stream1.next().await.is_none());
    assert!(stream2.next().await.is_none());
    assert_eq!(hub.size(None), 0);
}
