use super::cliente::{es_error_olvidar_panel, ClientePanel};
use super::instantanea::InstantaneaCola;
use crate::ui::panel::{vista_reproductor, vista_terminada};
use medios::Cancion;
use nucleo::idioma::Idiomas;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use twilight_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker};
use twilight_model::id::Id;

pub const PANEL_DEBOUNCE_MS: u64 = 750;

pub enum OrdenPanel {
    Enviar {
        canal_texto: Id<ChannelMarker>,
        instantanea: InstantaneaCola,
    },
    Refrescar {
        inmediato: bool,
        instantanea: InstantaneaCola,
    },
    Cerrar {
        titulo_key: String,
        descripcion_key: Option<String>,
        cancion: Option<Cancion>,
    },
    Reemplazar {
        mensaje_id: Id<MessageMarker>,
        canal_id: Id<ChannelMarker>,
    },
    Adoptar {
        mensaje_id: Id<MessageMarker>,
        canal_id: Id<ChannelMarker>,
    },
    EsActual {
        mensaje_id: Id<MessageMarker>,
        responder: oneshot::Sender<bool>,
    },
    IniciarRefresco {
        intervalo_segundos: u64,
    },
    DetenerRefresco,
}

pub struct ServidorPanel {
    pub guild_id: Id<GuildMarker>,
    pub canal_id: Option<Id<ChannelMarker>>,
    pub panel_id: Option<Id<MessageMarker>>,
    pub cliente: Arc<dyn ClientePanel>,
    pub idiomas: Arc<Idiomas>,
    pub lang: String,
}

impl ServidorPanel {
    pub fn nuevo(
        g: Id<GuildMarker>,
        cl: Arc<dyn ClientePanel>,
        idm: Arc<Idiomas>,
        l: String,
    ) -> Self {
        Self {
            guild_id: g,
            canal_id: None,
            panel_id: None,
            cliente: cl,
            idiomas: idm,
            lang: l,
        }
    }

    pub async fn procesar_orden(&mut self, orden: OrdenPanel) {
        match orden {
            OrdenPanel::Enviar {
                canal_texto,
                instantanea,
            } => self.enviar(canal_texto, &instantanea).await,
            OrdenPanel::Refrescar {
                inmediato: true,
                instantanea,
            } => self.refrescar_inmediato(&instantanea).await,
            OrdenPanel::Refrescar {
                inmediato: false, ..
            } => {}
            OrdenPanel::Cerrar {
                titulo_key,
                descripcion_key,
                cancion,
            } => {
                self.cerrar(&titulo_key, descripcion_key.as_deref(), cancion.as_ref())
                    .await
            }
            OrdenPanel::Reemplazar {
                mensaje_id,
                canal_id,
            } => self.reemplazar(mensaje_id, canal_id).await,
            OrdenPanel::Adoptar {
                mensaje_id,
                canal_id,
            } => self.adoptar(mensaje_id, canal_id),
            OrdenPanel::EsActual {
                mensaje_id,
                responder,
            } => {
                let _ = responder.send(self.panel_id == Some(mensaje_id));
            }
            OrdenPanel::IniciarRefresco { .. } | OrdenPanel::DetenerRefresco => {}
        }
    }

    async fn enviar(&mut self, canal_texto: Id<ChannelMarker>, snap: &InstantaneaCola) {
        let payload = vista_reproductor(&self.idiomas, &self.lang, snap);
        if let (Some(panel_id), Some(canal_id)) = (self.panel_id, self.canal_id) {
            if canal_id == canal_texto {
                let ultimo = self
                    .cliente
                    .obtener_ultimo_mensaje_id(canal_texto)
                    .await
                    .unwrap_or(None);
                if ultimo == Some(panel_id) {
                    match self
                        .cliente
                        .editar_panel(canal_texto, panel_id, &payload)
                        .await
                    {
                        Ok(()) => return,
                        Err(e) if es_error_olvidar_panel(&e) => self.panel_id = None,
                        Err(e) => {
                            tracing::warn!("Panel update failed in {}: {e}", self.guild_id);
                            return;
                        }
                    }
                }
            }
        }
        if let (Some(old_panel), Some(canal_id)) = (self.panel_id.take(), self.canal_id) {
            let cl = Arc::clone(&self.cliente);
            tokio::spawn(async move {
                let _ = cl.borrar_mensaje(canal_id, old_panel).await;
            });
        }
        match self.cliente.enviar_panel(canal_texto, &payload).await {
            Ok(new_id) => {
                self.panel_id = Some(new_id);
                self.canal_id = Some(canal_texto);
            }
            Err(e) => tracing::warn!("Panel update failed in {}: {e}", self.guild_id),
        }
    }

    async fn refrescar_inmediato(&mut self, snap: &InstantaneaCola) {
        if let (Some(panel_id), Some(canal_id)) = (self.panel_id, self.canal_id) {
            let payload = vista_reproductor(&self.idiomas, &self.lang, snap);
            if let Err(e) = self
                .cliente
                .editar_panel(canal_id, panel_id, &payload)
                .await
            {
                if es_error_olvidar_panel(&e) {
                    self.panel_id = None;
                }
                tracing::warn!("Panel update failed in {}: {e}", self.guild_id);
            }
        }
    }

    async fn cerrar(&mut self, tit: &str, desc: Option<&str>, cancion: Option<&Cancion>) {
        if let (Some(panel_id), Some(canal_id)) = (self.panel_id.take(), self.canal_id) {
            let payload = vista_terminada(&self.idiomas, &self.lang, tit, desc, cancion);
            let _ = self
                .cliente
                .editar_panel(canal_id, panel_id, &payload)
                .await;
        }
    }

    async fn reemplazar(&mut self, nuevo_id: Id<MessageMarker>, nuevo_canal: Id<ChannelMarker>) {
        let anterior = self.panel_id.replace(nuevo_id);
        self.canal_id = Some(nuevo_canal);
        if let Some(old) = anterior {
            if old != nuevo_id {
                let _ = self.cliente.borrar_mensaje(nuevo_canal, old).await;
            }
        }
    }

    fn adoptar(&mut self, nuevo_id: Id<MessageMarker>, nuevo_canal: Id<ChannelMarker>) {
        self.panel_id = Some(nuevo_id);
        self.canal_id = Some(nuevo_canal);
    }
}

pub async fn ejecutar_tarea_servidor(
    mut servidor: ServidorPanel,
    mut receptor: tokio::sync::mpsc::Receiver<OrdenPanel>,
) {
    let mut debounce: Option<Pin<Box<tokio::time::Sleep>>> = None;
    let mut pendiente: Option<InstantaneaCola> = None;
    let mut periodic_interval: Option<tokio::time::Interval> = None;

    loop {
        tokio::select! {
            orden = receptor.recv() => {
                let Some(orden) = orden else { break; };
                match orden {
                    OrdenPanel::Refrescar { inmediato: false, instantanea } => {
                        pendiente = Some(instantanea);
                        debounce = Some(Box::pin(tokio::time::sleep(Duration::from_millis(PANEL_DEBOUNCE_MS))));
                    }
                    OrdenPanel::IniciarRefresco { intervalo_segundos } => {
                        if intervalo_segundos > 0 {
                            let mut interval = tokio::time::interval(Duration::from_secs(intervalo_segundos));
                            interval.tick().await;
                            periodic_interval = Some(interval);
                        }
                    }
                    OrdenPanel::DetenerRefresco => { periodic_interval = None; }
                    OrdenPanel::Cerrar { .. } => {
                        debounce = None; pendiente = None; periodic_interval = None;
                        servidor.procesar_orden(orden).await;
                    }
                    OrdenPanel::Reemplazar { .. } | OrdenPanel::Adoptar { .. } => {
                        debounce = None; pendiente = None;
                        servidor.procesar_orden(orden).await;
                    }
                    _ => servidor.procesar_orden(orden).await,
                }
            }
            _ = async {
                if let Some(ref mut d) = debounce { d.as_mut().await; } else { futures_util::future::pending::<()>().await; }
            } => {
                debounce = None;
                if let Some(snap) = pendiente.take() { servidor.refrescar_inmediato(&snap).await; }
            }
            _ = async {
                if let Some(ref mut p) = periodic_interval { p.tick().await; } else { futures_util::future::pending::<()>().await; }
            } => {
                if let Some(snap) = &pendiente {
                    if !snap.pausada { servidor.refrescar_inmediato(snap).await; }
                }
            }
        }
    }
}
