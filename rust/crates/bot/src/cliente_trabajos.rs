use crate::acciones_voz::AccionesVozBot;
use crate::cliente::{DespachadorEventos, HooksReady, RouterInteraccion};
use player::{EstadoVoz, GestorPresencia};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::{sync::mpsc, task::JoinSet};
use twilight_gateway::{error::ReceiveMessageErrorType, EventTypeFlags, Shard, StreamExt};

pub(crate) const MAX_TAREAS_GATEWAY: usize = 64;

pub(crate) struct TrabajosGateway {
    pub tareas: JoinSet<()>,
    pub presencia: Option<mpsc::Sender<(String, EstadoVoz)>>,
}

impl TrabajosGateway {
    pub fn nuevo(
        presencia: Option<Arc<GestorPresencia>>,
        acciones: Option<Arc<AccionesVozBot>>,
    ) -> Self {
        let mut tareas = JoinSet::new();
        let presencia = presencia.zip(acciones).map(|(gestor, acciones)| {
            let (tx, mut rx) = mpsc::channel::<(String, EstadoVoz)>(4096);
            tareas.spawn(async move {
                while let Some((guild, estado)) = rx.recv().await {
                    let pausada = acciones.cola_esta_pausada(&guild).await;
                    gestor
                        .actualizar_voz(&guild, estado, pausada, acciones.clone())
                        .await;
                }
            });
            tx
        });
        Self { tareas, presencia }
    }
}

pub async fn ejecutar_bucle_eventos<H: HooksReady + 'static, R: RouterInteraccion + 'static>(
    mut shard: Shard,
    despachador: Arc<DespachadorEventos<H, R>>,
    apagando: Arc<AtomicBool>,
) -> Result<(), String> {
    let mut trabajos = TrabajosGateway::nuevo(
        despachador.gestor_presencia.clone(),
        despachador.acciones_voz.clone(),
    );
    let flags = EventTypeFlags::READY
        | EventTypeFlags::GUILD_CREATE
        | EventTypeFlags::GUILD_UPDATE
        | EventTypeFlags::INTERACTION_CREATE
        | EventTypeFlags::VOICE_STATE_UPDATE
        | EventTypeFlags::VOICE_SERVER_UPDATE
        | EventTypeFlags::RESUMED;

    while !apagando.load(Ordering::SeqCst) {
        if let Some(avg) = shard.latency().average() {
            despachador
                .latencia
                .store(avg.as_millis() as u64, Ordering::Relaxed);
        }

        let recibido = tokio::select! {
            evento = shard.next_event(flags) => evento,
            Some(tarea) = trabajos.tareas.join_next(), if !trabajos.tareas.is_empty() => {
                if let Err(e) = tarea {
                    tracing::error!("Falló una tarea del gateway: {e}");
                }
                continue;
            }
        };
        let evento = match recibido {
            Some(Ok(ev)) => ev,
            Some(Err(err)) => {
                tracing::warn!("Error al recibir mensaje de gateway: {}", err);
                if matches!(err.kind(), ReceiveMessageErrorType::Reconnect) {
                    tracing::info!("Intentando reconectar al gateway...");
                }
                continue;
            }
            None => {
                if apagando.load(Ordering::SeqCst) {
                    break;
                }
                return Err("Conexión con el gateway cerrada de forma imprevista".into());
            }
        };

        despachador.procesar_evento(evento, &mut trabajos).await?;
    }
    Ok(())
}
