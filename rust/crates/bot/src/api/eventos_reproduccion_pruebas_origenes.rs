//! Pruebas de origen: comando, avance, restauración, corte prematuro y fallo del registrador.

use super::*;

#[tokio::test]
async fn origen_comando_registra_una_vez() {
    let reg = Arc::new(RegistradorFalso::default());
    let (driver, colas, _prov) = montar(reg.clone()).await;
    encolar(&colas, &["A"]).await;
    let cola_arc = colas.obtener_cola("10").await;
    {
        let mut cola = cola_arc.lock().await;
        player::reproduccion::iniciar_pista_con_settings(
            &driver,
            &mut cola,
            "yt-dlp",
            Vec::new(),
            &medios::YtDlpSettings::default(),
        )
        .await
        .unwrap();
    }
    esperar(&reg, 1).await;
    assert_eq!(titulos(&reg), vec!["A"]);
}

/// Avance de cola: al saltar, el listener arranca la siguiente pista con "advance" y registra una vez.
#[tokio::test]
async fn origen_avance_registra_una_vez_por_pista() {
    let reg = Arc::new(RegistradorFalso::default());
    let (driver, colas, _prov) = montar(reg.clone()).await;
    encolar(&colas, &["A", "B"]).await;
    let cola_arc = colas.obtener_cola("10").await;
    {
        let mut cola = cola_arc.lock().await;
        player::reproduccion::iniciar_pista_con_settings(
            &driver,
            &mut cola,
            "yt-dlp",
            Vec::new(),
            &medios::YtDlpSettings::default(),
        )
        .await
        .unwrap();
    }
    esperar(&reg, 1).await;
    {
        // Un salto quita la cabecera antes de avanzar; el listener ya registró A.
        let mut cola = cola_arc.lock().await;
        cola.canciones.remove(0);
    }
    driver.detener_y_avanzar(10).await.unwrap();
    esperar(&reg, 2).await;
    assert_eq!(titulos(&reg), vec!["A", "B"]);
}

/// Restauración: restauracion.rs llama a iniciar_pista_desde_con_settings con la posición guardada.
#[tokio::test]
async fn origen_restauracion_registra_una_vez() {
    let reg = Arc::new(RegistradorFalso::default());
    let (driver, colas, _prov) = montar(reg.clone()).await;
    encolar(&colas, &["A"]).await;
    let cola_arc = colas.obtener_cola("10").await;
    {
        let mut cola = cola_arc.lock().await;
        player::reproduccion::iniciar_pista_desde_con_settings(
            &driver,
            &mut cola,
            "yt-dlp",
            Vec::new(),
            Some(5),
            &medios::YtDlpSettings::default(),
        )
        .await
        .unwrap();
    }
    esperar(&reg, 1).await;
    assert_eq!(titulos(&reg), vec!["A"]);
}

/// Corte prematuro: Node (events.js FINISH_SONG) fuerza repetición de canción y reinicia con seek;
/// su PLAY_SONG vuelve a llamar a recordPlay. Rust hace lo mismo: el reinicio emite "play" y registra
/// otra vez. Esta prueba fija esa paridad, incluido el duplicado.
#[tokio::test]
async fn corte_prematuro_registra_dos_veces_como_node() {
    let reg = Arc::new(RegistradorFalso::default());
    let (driver, colas, _prov) = montar(reg.clone()).await;
    encolar(&colas, &["A"]).await;
    let cola_arc = colas.obtener_cola("10").await;
    {
        let mut cola = cola_arc.lock().await;
        cola.canciones[0].cancion.duracion = 200;
        player::reproduccion::iniciar_pista_con_settings(
            &driver,
            &mut cola,
            "yt-dlp",
            Vec::new(),
            &medios::YtDlpSettings::default(),
        )
        .await
        .unwrap();
    }
    esperar(&reg, 1).await;
    {
        let mut cola = cola_arc.lock().await;
        cola.posicion_segundos = 50;
        let accion = player::al_terminar_cancion(&mut cola);
        assert!(
            accion.is_some(),
            "el corte a 50 s de 200 s debe recuperarse"
        );
        assert_eq!(cola.modo_repeticion, player::ModoRepeticion::Cancion);
        let seek = cola.posicion_segundos;
        player::reproduccion::iniciar_pista_desde_con_settings(
            &driver,
            &mut cola,
            "yt-dlp",
            Vec::new(),
            Some(seek),
            &medios::YtDlpSettings::default(),
        )
        .await
        .unwrap();
    }
    esperar(&reg, 2).await;
    assert_eq!(titulos(&reg), vec!["A", "A"]);
}

/// Un fallo del registrador no detiene la reproducción: la pista siguiente arranca y también se registra.
#[tokio::test]
async fn fallo_de_historial_no_detiene_la_reproduccion() {
    let reg = Arc::new(RegistradorFalso {
        falla: true,
        ..Default::default()
    });
    let (driver, colas, _prov) = montar(reg.clone()).await;
    encolar(&colas, &["A", "B"]).await;
    let cola_arc = colas.obtener_cola("10").await;
    {
        let mut cola = cola_arc.lock().await;
        player::reproduccion::iniciar_pista_con_settings(
            &driver,
            &mut cola,
            "yt-dlp",
            Vec::new(),
            &medios::YtDlpSettings::default(),
        )
        .await
        .unwrap();
    }
    esperar(&reg, 1).await;
    {
        let mut cola = cola_arc.lock().await;
        cola.canciones.remove(0);
    }
    driver.detener_y_avanzar(10).await.unwrap();
    esperar(&reg, 2).await;
    assert_eq!(titulos(&reg), vec!["A", "B"]);
    assert!(
        driver.active_track(10).is_some(),
        "la reproducción debe seguir"
    );
}
