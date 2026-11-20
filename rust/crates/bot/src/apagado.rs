//! Coordinador de apagado en orden (Spec S-21).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use twilight_gateway::CloseFrame;

pub const SHUTDOWN_GRACE_MS: u64 = 10_000;

#[async_trait::async_trait]
pub trait PasosApagado: Send + Sync {
    async fn dejar_despachar_eventos(&self) -> Result<(), String>;
    async fn cerrar_api_y_tunel(&self) -> Result<(), String>;
    async fn apagar_reproductor(&self) -> Result<(), String>;
    async fn volcar_csv_voz(&self) -> Result<(), String>;
    async fn cerrar_gateway(&self) -> Result<(), String>;
}

pub async fn ejecutar_apagado<P: PasosApagado>(pasos: &P, tiempo_limite: Duration) -> i32 {
    let res = tokio::time::timeout(tiempo_limite, async {
        tracing::info!("Iniciando secuencia de apagado...");
        if let Err(e) = pasos.dejar_despachar_eventos().await {
            tracing::warn!("Error al detener despacho de eventos: {}", e);
        }
        if let Err(e) = pasos.cerrar_api_y_tunel().await {
            tracing::warn!("Error al cerrar API y túnel: {}", e);
        }
        if let Err(e) = pasos.apagar_reproductor().await {
            tracing::error!("Error al apagar reproductor: {}", e);
        }
        if let Err(e) = pasos.volcar_csv_voz().await {
            tracing::warn!("Error al volcar registros de voz en CSV: {}", e);
        }
        if let Err(e) = pasos.cerrar_gateway().await {
            tracing::warn!("Error al cerrar gateway: {}", e);
        }
        tracing::info!("Apagado completado correctamente.");
    })
    .await;

    match res {
        Ok(()) => 0,
        Err(_) => {
            tracing::warn!(
                "Tiempo límite de gracia de apagado superado ({:?}); forzando salida limpia",
                tiempo_limite
            );
            0
        }
    }
}

pub async fn ejecutar_apagado_critico<P: PasosApagado>(pasos: &P, tiempo_limite: Duration) -> i32 {
    let _ = ejecutar_apagado(pasos, tiempo_limite).await;
    1
}

pub struct PasosApagadoReal {
    pub apagando: Arc<AtomicBool>,
    pub gestor_colas: Option<Arc<player::GestorColas>>,
    pub almacen_sesiones: Option<Arc<player::AlmacenSesiones>>,
    pub exportador_csv: Option<Arc<voice::ExportadorCsvVoz>>,
    pub driver: Option<Arc<player::SongbirdDriver>>,
    pub sender: Option<twilight_gateway::MessageSender>,
}

#[async_trait::async_trait]
impl PasosApagado for PasosApagadoReal {
    async fn dejar_despachar_eventos(&self) -> Result<(), String> {
        self.apagando.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn cerrar_api_y_tunel(&self) -> Result<(), String> {
        // En F6, ServiciosApiBot gestiona el apagado coordinado del servidor API, túnel y anunciador.
        Ok(())
    }

    async fn apagar_reproductor(&self) -> Result<(), String> {
        if let (Some(colas), Some(almacen)) = (&self.gestor_colas, &self.almacen_sesiones) {
            player::guardar_todas_las_sesiones(colas.as_ref(), almacen.as_ref()).await;
            if let Some(drv) = &self.driver {
                for c in colas.colas_activas().await {
                    let cola = c.lock().await;
                    if let Ok(gid) = cola.guild_id.parse::<u64>() {
                        let _ = drv.leave(gid).await;
                    }
                }
            }
        }
        Ok(())
    }

    async fn volcar_csv_voz(&self) -> Result<(), String> {
        if let Some(csv) = &self.exportador_csv {
            csv.flush_all();
        }
        Ok(())
    }

    async fn cerrar_gateway(&self) -> Result<(), String> {
        if let Some(s) = &self.sender {
            let _ = s.close(CloseFrame::NORMAL);
        }
        Ok(())
    }
}
