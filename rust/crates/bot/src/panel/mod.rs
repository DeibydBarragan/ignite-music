pub mod acciones_inmediatas;
pub mod cliente;
pub mod instantanea;
pub mod interaccion;
pub mod modal;
pub mod nowplaying;
pub mod servidor;

pub use cliente::{es_error_olvidar_panel, ClientePanel, ClientePanelHttp, ClientePanelNulo};
pub use instantanea::InstantaneaCola;
pub use interaccion::{manejar_interaccion_panel, ContextoPanel};
pub use nowplaying::ejecutar_nowplaying;
pub use servidor::{OrdenPanel, ServidorPanel, PANEL_DEBOUNCE_MS};

use medios::Cancion;
use nucleo::idioma::Idiomas;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use twilight_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker};
use twilight_model::id::Id;

#[derive(Clone)]
pub struct GestorPaneles {
    canales: Arc<Mutex<HashMap<Id<GuildMarker>, mpsc::Sender<OrdenPanel>>>>,
    cliente: Arc<dyn ClientePanel>,
    idiomas: Arc<Idiomas>,
    contexto_api: Option<crate::api::EstadoApiContexto>,
}

impl GestorPaneles {
    pub fn nuevo(cliente: Arc<dyn ClientePanel>, idiomas: Arc<Idiomas>) -> Self {
        Self {
            canales: Arc::new(Mutex::new(HashMap::new())),
            cliente,
            idiomas,
            contexto_api: None,
        }
    }

    pub fn con_contexto_api(mut self, contexto: crate::api::EstadoApiContexto) -> Self {
        self.contexto_api = Some(contexto);
        self
    }

    async fn obtener_o_crear_canal(
        &self,
        guild_id: Id<GuildMarker>,
        lang: &str,
    ) -> mpsc::Sender<OrdenPanel> {
        let mut map = self.canales.lock().await;
        if let Some(tx) = map.get(&guild_id) {
            if !tx.is_closed() {
                return tx.clone();
            }
        }
        let (tx, rx) = mpsc::channel(32);
        let servidor = ServidorPanel::nuevo(
            guild_id,
            Arc::clone(&self.cliente),
            Arc::clone(&self.idiomas),
            lang.to_string(),
        );
        tokio::spawn(servidor::ejecutar_tarea_servidor(servidor, rx));
        map.insert(guild_id, tx.clone());
        tx
    }

    pub async fn enviar_panel(
        &self,
        guild_id: Id<GuildMarker>,
        canal_texto: Id<ChannelMarker>,
        instantanea: InstantaneaCola,
        lang: &str,
    ) {
        let tx = self.obtener_o_crear_canal(guild_id, lang).await;
        let _ = tx
            .send(OrdenPanel::Enviar {
                canal_texto,
                instantanea,
            })
            .await;
    }

    pub async fn refrescar_panel(
        &self,
        guild_id: Id<GuildMarker>,
        instantanea: InstantaneaCola,
        inmediato: bool,
        lang: &str,
    ) {
        if let Some(ctx) = &self.contexto_api {
            ctx.sse
                .publicar_sincronizacion(&guild_id.to_string(), ctx)
                .await;
        }
        let tx = self.obtener_o_crear_canal(guild_id, lang).await;
        let _ = tx
            .send(OrdenPanel::Refrescar {
                inmediato,
                instantanea,
            })
            .await;
    }

    pub async fn cerrar_panel(
        &self,
        guild_id: Id<GuildMarker>,
        titulo_key: &str,
        descripcion_key: Option<&str>,
        cancion: Option<Cancion>,
        lang: &str,
    ) {
        if let Some(ctx) = &self.contexto_api {
            ctx.sse
                .publicar_sincronizacion(&guild_id.to_string(), ctx)
                .await;
        }
        let tx = self.obtener_o_crear_canal(guild_id, lang).await;
        let _ = tx
            .send(OrdenPanel::Cerrar {
                titulo_key: titulo_key.to_string(),
                descripcion_key: descripcion_key.map(|s| s.to_string()),
                cancion,
            })
            .await;
    }

    pub async fn reemplazar_panel(
        &self,
        guild_id: Id<GuildMarker>,
        canal_id: Id<ChannelMarker>,
        mensaje_id: Id<MessageMarker>,
        lang: &str,
    ) {
        let tx = self.obtener_o_crear_canal(guild_id, lang).await;
        let _ = tx
            .send(OrdenPanel::Reemplazar {
                mensaje_id,
                canal_id,
            })
            .await;
    }

    pub async fn adoptar_panel(
        &self,
        guild_id: Id<GuildMarker>,
        canal_id: Id<ChannelMarker>,
        mensaje_id: Id<MessageMarker>,
        lang: &str,
    ) {
        let tx = self.obtener_o_crear_canal(guild_id, lang).await;
        let _ = tx
            .send(OrdenPanel::Adoptar {
                mensaje_id,
                canal_id,
            })
            .await;
    }

    pub async fn es_panel_actual(
        &self,
        guild_id: Id<GuildMarker>,
        mensaje_id: Id<MessageMarker>,
        lang: &str,
    ) -> bool {
        let tx = self.obtener_o_crear_canal(guild_id, lang).await;
        let (responder, rx) = oneshot::channel();
        if tx
            .send(OrdenPanel::EsActual {
                mensaje_id,
                responder,
            })
            .await
            .is_err()
        {
            return false;
        }
        rx.await.unwrap_or(false)
    }

    pub async fn iniciar_refresco(
        &self,
        guild_id: Id<GuildMarker>,
        intervalo_segundos: u64,
        lang: &str,
    ) {
        let tx = self.obtener_o_crear_canal(guild_id, lang).await;
        let _ = tx
            .send(OrdenPanel::IniciarRefresco { intervalo_segundos })
            .await;
    }

    pub async fn detener_refresco(&self, guild_id: Id<GuildMarker>, lang: &str) {
        let tx = self.obtener_o_crear_canal(guild_id, lang).await;
        let _ = tx.send(OrdenPanel::DetenerRefresco).await;
    }
}
