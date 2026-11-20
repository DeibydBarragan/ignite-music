//! Orden de apagado: el guardado de sesiones no depende de anunciador, túnel ni API (Spec S-21, F2).

use bot::apagado::PasosApagado;
use bot::arranque::apagar_con_servicios;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct PasosDoble {
    log: Mutex<Vec<&'static str>>,
}

impl PasosDoble {
    fn registrar(&self, paso: &'static str) -> Result<(), String> {
        self.log.lock().unwrap().push(paso);
        Ok(())
    }
}

#[async_trait::async_trait]
impl PasosApagado for PasosDoble {
    async fn dejar_despachar_eventos(&self) -> Result<(), String> {
        self.registrar("despacho")
    }
    async fn cerrar_api_y_tunel(&self) -> Result<(), String> {
        Ok(())
    }
    async fn apagar_reproductor(&self) -> Result<(), String> {
        self.registrar("sesiones")
    }
    async fn volcar_csv_voz(&self) -> Result<(), String> {
        self.registrar("csv")
    }
    async fn cerrar_gateway(&self) -> Result<(), String> {
        self.registrar("gateway")
    }
}

#[tokio::test]
async fn tunel_colgado_no_impide_guardar_sesiones() {
    let pasos = PasosDoble::default();
    let inicio = Instant::now();
    let codigo = apagar_con_servicios(
        &pasos,
        Duration::from_secs(2),
        Duration::from_millis(100),
        std::future::pending::<()>(),
    )
    .await;
    assert_eq!(codigo, 0);
    assert!(inicio.elapsed() < Duration::from_secs(1));
    assert!(pasos.log.lock().unwrap().contains(&"sesiones"));
}

#[tokio::test]
async fn apagado_normal_guarda_y_cierra_todo() {
    let pasos = PasosDoble::default();
    let servicios_cerrados = Arc::new(AtomicBool::new(false));
    let marca = servicios_cerrados.clone();
    let codigo = apagar_con_servicios(
        &pasos,
        Duration::from_secs(2),
        Duration::from_secs(1),
        async move { marca.store(true, Ordering::SeqCst) },
    )
    .await;
    assert_eq!(codigo, 0);
    assert!(servicios_cerrados.load(Ordering::SeqCst));
    let log = pasos.log.lock().unwrap().clone();
    assert_eq!(log, vec!["despacho", "sesiones", "csv", "gateway"]);
}
