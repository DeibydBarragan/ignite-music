use medios::{Limitador, LimitadorError, Prioridad};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn cancelar_despues_de_recibir_el_turno_libera_el_permiso() {
    let limitador = Limitador::new(1);
    let primero = limitador.acquire(Prioridad::Alta, None).await.unwrap();
    let copia = limitador.clone();
    let espera = tokio::spawn(async move { copia.acquire(Prioridad::Alta, None).await });
    tokio::task::yield_now().await;
    assert_eq!(limitador.pending_high(), 1);
    drop(primero);
    espera.abort();
    let _ = espera.await;
    assert_eq!(limitador.active(), 0);
    let nuevo = limitador
        .acquire(Prioridad::Alta, Some(Duration::from_millis(100)))
        .await
        .unwrap();
    drop(nuevo);
}

#[tokio::test]
async fn copias_de_configuracion_comparten_los_dos_permisos() {
    let settings = medios::YtDlpSettings {
        concurrency: 2,
        limiter: Limitador::new(2),
        ..Default::default()
    };
    let busqueda = settings.clone();
    let autoplay = settings.clone();
    let primero = settings
        .limiter
        .acquire(Prioridad::Alta, None)
        .await
        .unwrap();
    let segundo = busqueda
        .limiter
        .acquire(Prioridad::Alta, None)
        .await
        .unwrap();
    let copia = autoplay.limiter.clone();
    let espera = tokio::spawn(async move {
        copia
            .acquire(Prioridad::Baja, Some(Duration::from_secs(1)))
            .await
    });
    tokio::task::yield_now().await;
    assert_eq!(settings.limiter.active(), 2);
    assert_eq!(settings.limiter.pending_low(), 1);
    drop(primero);
    let tercero = espera.await.unwrap().unwrap();
    assert_eq!(settings.limiter.active(), 2);
    drop((segundo, tercero));
    assert_eq!(settings.limiter.active(), 0);
}
#[tokio::test]
async fn test_limit_is_respected() {
    let limitador = Limitador::new(2);
    assert_eq!(limitador.limit(), 2);
    assert_eq!(limitador.active(), 0);

    let p1 = limitador.acquire(Prioridad::Alta, None).await.unwrap();
    assert_eq!(limitador.active(), 1);

    let p2 = limitador.acquire(Prioridad::Baja, None).await.unwrap();
    assert_eq!(limitador.active(), 2);

    drop(p1);
    assert_eq!(limitador.active(), 1);

    drop(p2);
    assert_eq!(limitador.active(), 0);
}

#[tokio::test]
async fn carga_combinada_busqueda_lista_y_fuente_activa_respeta_concurrencia_dos() {
    use medios::{buscar_cancion, resolve_playlist, stream_info, FuentesCache, YtDlpSettings};
    use std::sync::atomic::AtomicBool;

    std::env::set_var("FAKE_YTDLP_DELAY_MS", "120");
    let mut vars = std::collections::HashMap::new();
    vars.insert("YTDLP_CONCURRENCY".to_string(), "2".to_string());
    let config = nucleo::Config::from_map(std::path::PathBuf::new(), &vars);
    let concurrency = config.ytdlp.concurrency as usize;
    let limiter = Limitador::new(concurrency);
    let settings = YtDlpSettings {
        concurrency,
        limiter: limiter.clone(),
        ..Default::default()
    };
    let cache = FuentesCache::new();
    let fake_bin = env!("CARGO_BIN_EXE_fake-ytdlp");

    // Dos fuentes activas retienen permisos low como ProcesoAudio hasta que terminan.
    let fuentes_activas: Vec<_> = (0..2)
        .map(|_| {
            let limiter = limiter.clone();
            tokio::spawn(async move {
                let _permiso = limiter
                    .acquire(Prioridad::Baja, Some(Duration::from_secs(2)))
                    .await
                    .unwrap();
                tokio::time::sleep(Duration::from_millis(180)).await;
            })
        })
        .collect();
    while limiter.active() < 2 {
        tokio::task::yield_now().await;
    }

    let terminado = Arc::new(AtomicBool::new(false));
    let monitor_fin = terminado.clone();
    let monitor_limiter = limiter.clone();
    let monitor = tokio::spawn(async move {
        let mut maximo = 0;
        while !monitor_fin.load(Ordering::SeqCst) {
            maximo = maximo.max(monitor_limiter.active());
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        maximo.max(monitor_limiter.active())
    });

    let busqueda = buscar_cancion(
        fake_bin,
        "carga de concurrencia",
        Some("guild-load"),
        &settings,
        &[],
        &limiter,
        Prioridad::Alta,
        None,
        Some(&cache),
    );
    let lista = resolve_playlist(
        "https://www.youtube.com/playlist?list=PL1",
        None,
        Some(1),
        None,
        fake_bin,
        Some("guild-load"),
        &settings,
        &[],
        &limiter,
    );
    let info = stream_info(
        fake_bin,
        "https://youtu.be/abcdefghijk",
        Some("guild-load"),
        &settings,
        &[],
        &limiter,
        Prioridad::Alta,
        &cache,
    );

    let (busqueda, lista, info) = tokio::join!(busqueda, lista, info);
    terminado.store(true, Ordering::SeqCst);
    let maximo = monitor.await.unwrap();
    assert!(busqueda.unwrap().is_some());
    assert_eq!(lista.unwrap().tracks.len(), 1);
    assert_eq!(info.unwrap()["id"], "abcdefghijk");
    for fuente in fuentes_activas {
        fuente.await.unwrap();
    }
    assert_eq!(
        maximo, 2,
        "La carga debe alcanzar, pero nunca superar, dos slots"
    );
    assert_eq!(limiter.active(), 0, "No deben quedar permisos retenidos");
    std::env::remove_var("FAKE_YTDLP_DELAY_MS");
}

#[tokio::test]
async fn test_high_priority_before_low_priority() {
    let limitador = Limitador::new(1);
    let p0 = limitador.acquire(Prioridad::Alta, None).await.unwrap();

    let order = Arc::new(tokio::sync::Mutex::new(Vec::new()));

    // Queue low 1 and low 2
    let l1 = limitador.clone();
    let o1 = order.clone();
    tokio::spawn(async move {
        let _p = l1.acquire(Prioridad::Baja, None).await.unwrap();
        o1.lock().await.push("low_1");
    });

    let l2 = limitador.clone();
    let o2 = order.clone();
    tokio::spawn(async move {
        let _p = l2.acquire(Prioridad::Baja, None).await.unwrap();
        o2.lock().await.push("low_2");
    });

    tokio::time::sleep(Duration::from_millis(20)).await;

    // Queue high 1
    let h1 = limitador.clone();
    let oh = order.clone();
    tokio::spawn(async move {
        let _p = h1.acquire(Prioridad::Alta, None).await.unwrap();
        oh.lock().await.push("high_1");
    });

    tokio::time::sleep(Duration::from_millis(20)).await;

    assert_eq!(limitador.pending_low(), 2);
    assert_eq!(limitador.pending_high(), 1);

    // Release p0 -> high_1 must be served first!
    drop(p0);

    tokio::time::sleep(Duration::from_millis(50)).await;

    let res = order.lock().await.clone();
    assert_eq!(res.first().copied(), Some("high_1"));
}

#[tokio::test]
async fn test_alternates_turns_when_both_lanes_wait() {
    let limitador = Limitador::new(1);
    let p0 = limitador.acquire(Prioridad::Alta, None).await.unwrap();

    let history = Arc::new(tokio::sync::Mutex::new(Vec::new()));

    // Queue two high and two low
    for tag in ["high_1", "high_2"] {
        let lim = limitador.clone();
        let hist = history.clone();
        tokio::spawn(async move {
            let p = lim.acquire(Prioridad::Alta, None).await.unwrap();
            hist.lock().await.push(tag);
            tokio::time::sleep(Duration::from_millis(15)).await;
            drop(p);
        });
    }

    for tag in ["low_1", "low_2"] {
        let lim = limitador.clone();
        let hist = history.clone();
        tokio::spawn(async move {
            let p = lim.acquire(Prioridad::Baja, None).await.unwrap();
            hist.lock().await.push(tag);
            tokio::time::sleep(Duration::from_millis(15)).await;
            drop(p);
        });
    }

    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(limitador.pending_high(), 2);
    assert_eq!(limitador.pending_low(), 2);

    drop(p0);

    tokio::time::sleep(Duration::from_millis(200)).await;

    let recorded = history.lock().await.clone();
    // Alternation: high_1 -> low_1 -> high_2 -> low_2
    assert_eq!(recorded, vec!["high_1", "low_1", "high_2", "low_2"]);
}

#[tokio::test]
async fn test_timeout_cancels_waiter_and_cleans_queue() {
    let limitador = Limitador::new(1);
    let p0 = limitador.acquire(Prioridad::Alta, None).await.unwrap();

    let err = limitador
        .acquire(Prioridad::Alta, Some(Duration::from_millis(50)))
        .await
        .unwrap_err();

    assert!(matches!(err, LimitadorError::TiempoAgotado(_)));
    assert_eq!(limitador.pending_high(), 0);

    drop(p0);
    assert_eq!(limitador.active(), 0);
}

#[tokio::test]
async fn test_cancelled_waiter_is_skipped_and_next_served() {
    let limitador = Limitador::new(1);
    let p0 = limitador.acquire(Prioridad::Alta, None).await.unwrap();

    // Waiter with short timeout
    let lim_timeout = limitador.clone();
    tokio::spawn(async move {
        let _ = lim_timeout
            .acquire(Prioridad::Alta, Some(Duration::from_millis(20)))
            .await;
    });

    // Waiter with no timeout
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();
    let lim_ok = limitador.clone();
    tokio::spawn(async move {
        let _p = lim_ok.acquire(Prioridad::Alta, None).await.unwrap();
        c.fetch_add(1, Ordering::SeqCst);
    });

    // Let the first waiter timeout
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Release p0 -> second waiter must acquire slot
    drop(p0);

    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}
