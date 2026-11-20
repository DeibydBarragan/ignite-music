//! Ciclo de vida y arranque del bot (Spec S-21, S-22).

use crate::cliente::HooksReady;
use crate::restauracion::ProveedorRestauracionReal;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct HooksBotReal {
    pub gestor_colas: Arc<player::GestorColas>,
    pub almacen_sesiones: Arc<player::AlmacenSesiones>,
    pub command_ids: Arc<RwLock<HashMap<String, String>>>,
    pub token: String,
    pub client_id: String,
    pub guild_id: Option<String>,
    pub proveedor_restauracion: Arc<ProveedorRestauracionReal>,
}

#[async_trait::async_trait]
impl HooksReady for HooksBotReal {
    async fn al_conectar(&self, _tag: &str, _servidores: usize) {}
    async fn cargar_comandos(&self) {
        match crate::comandos::registro::cargar_ids_comandos(
            &self.token,
            &self.client_id,
            self.guild_id.as_deref(),
        )
        .await
        {
            Ok(ids) => {
                let n = ids.len();
                *self.command_ids.write().await = ids;
                tracing::debug!("IDs de {} comandos cargados", n);
            }
            Err(e) => tracing::warn!("No se pudieron obtener IDs de comandos: {}", e),
        }
    }
    async fn restaurar_sesiones(&self) {
        let n = player::sesiones_restaurar::restaurar_sesiones(
            self.gestor_colas.as_ref(),
            self.almacen_sesiones.as_ref(),
            self.proveedor_restauracion.as_ref(),
        )
        .await;
        if n > 0 {
            tracing::info!("Restauradas {} sesiones musicales", n);
        }
    }
    async fn iniciar_servicios(&self) {
        tracing::debug!("Servicios API y anunciador preparados para S-26/S-29");
    }
}

pub async fn esperar_senal_terminacion() {
    #[cfg(windows)]
    {
        use tokio::signal::windows;
        let c = windows::ctrl_c();
        let b = windows::ctrl_break();
        let cl = windows::ctrl_close();
        if let (Ok(mut c), Ok(mut b), Ok(mut cl)) = (c, b, cl) {
            tokio::select! {
                _ = c.recv() => tracing::info!("Señal Ctrl+C recibida"),
                _ = b.recv() => tracing::info!("Señal Ctrl+Break recibida"),
                _ = cl.recv() => tracing::info!("Señal de cierre recibida"),
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("Señal Ctrl+C recibida");
}

/// Plazo de anunciador, túnel y API durante el apagado. Va aparte del guardado de sesiones.
pub const PLAZO_SERVICIOS_MS: u64 = 6_000;

/// Guarda sesiones y cierra servicios a la vez. Las sesiones empiezan al instante y no esperan
/// a anunciador, túnel ni API; tienen el plazo `grace` completo. Los servicios se cortan a
/// `plazo_servicios`, para que un túnel colgado no retrase el cierre.
/// shortcut: el gateway se cierra en paralelo con la API, no después de ella; si el dashboard
/// necesita la API cerrada antes, pasar a secuencial dentro de `ejecutar_apagado`.
pub async fn apagar_con_servicios<P, F>(
    pasos: &P,
    grace: std::time::Duration,
    plazo_servicios: std::time::Duration,
    servicios: F,
) -> i32
where
    P: crate::apagado::PasosApagado,
    F: std::future::Future<Output = ()>,
{
    let cerrar_servicios = async {
        if tokio::time::timeout(plazo_servicios, servicios)
            .await
            .is_err()
        {
            tracing::warn!(
                "Servicios de apagado superaron {:?}; se continúa",
                plazo_servicios
            );
        }
    };
    let (codigo, ()) = tokio::join!(
        crate::apagado::ejecutar_apagado(pasos, grace),
        cerrar_servicios
    );
    codigo
}

pub async fn supervisar_gateway(
    mut gateway: tokio::task::JoinHandle<Result<(), String>>,
    mut servicios: crate::api::ServiciosApiBot,
    pasos: &crate::apagado::PasosApagadoReal,
) -> i32 {
    let fallo_gateway = tokio::select! {
        _ = esperar_senal_terminacion() => false,
        salida = &mut gateway => {
            tracing::error!(?salida, "La tarea de gateway terminó antes del apagado");
            true
        }
    };
    pasos
        .apagando
        .store(true, std::sync::atomic::Ordering::SeqCst);
    servicios.hub_sse.cerrar();
    let grace = std::time::Duration::from_millis(crate::apagado::SHUTDOWN_GRACE_MS);
    let plazo_servicios = std::time::Duration::from_millis(PLAZO_SERVICIOS_MS);
    let salida = tokio::time::timeout(
        grace,
        apagar_con_servicios(pasos, grace, plazo_servicios, servicios.apagar()),
    )
    .await;
    if !fallo_gateway {
        gateway.abort();
        let _ = gateway.await;
    }
    let codigo = match salida {
        Ok(codigo) if !fallo_gateway => codigo,
        _ => 1,
    };
    if codigo == 0 {
        tracing::info!("Apagado completado de forma limpia");
    } else {
        tracing::error!(
            codigo,
            "Apagado incompleto o iniciado por fallo del gateway"
        );
    }
    codigo
}
