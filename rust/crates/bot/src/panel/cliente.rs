use crate::respuesta::{ErrorRespuesta, PayloadRespuesta};
use std::sync::Arc;
use twilight_http::Client as HttpClient;
use twilight_model::id::marker::{ChannelMarker, MessageMarker};
use twilight_model::id::Id;

pub fn es_error_olvidar_panel(err: &ErrorRespuesta) -> bool {
    matches!(
        err,
        ErrorRespuesta::CodigoDiscord(10008 | 10003 | 50001 | 50013)
    )
}

#[async_trait::async_trait]
pub trait ClientePanel: Send + Sync {
    async fn obtener_ultimo_mensaje_id(
        &self,
        c: Id<ChannelMarker>,
    ) -> Result<Option<Id<MessageMarker>>, ErrorRespuesta>;

    async fn enviar_panel(
        &self,
        c: Id<ChannelMarker>,
        p: &PayloadRespuesta,
    ) -> Result<Id<MessageMarker>, ErrorRespuesta>;

    async fn editar_panel(
        &self,
        c: Id<ChannelMarker>,
        m: Id<MessageMarker>,
        p: &PayloadRespuesta,
    ) -> Result<(), ErrorRespuesta>;

    async fn borrar_mensaje(
        &self,
        c: Id<ChannelMarker>,
        m: Id<MessageMarker>,
    ) -> Result<(), ErrorRespuesta>;
}

pub struct ClientePanelHttp {
    pub http: Arc<HttpClient>,
}

impl ClientePanelHttp {
    pub fn nuevo(http: Arc<HttpClient>) -> Self {
        Self { http }
    }
}

#[async_trait::async_trait]
impl ClientePanel for ClientePanelHttp {
    async fn obtener_ultimo_mensaje_id(
        &self,
        canal_id: Id<ChannelMarker>,
    ) -> Result<Option<Id<MessageMarker>>, ErrorRespuesta> {
        let msgs = self
            .http
            .channel_messages(canal_id)
            .limit(1)
            .await?
            .models()
            .await?;
        Ok(msgs.first().map(|m| m.id))
    }

    async fn enviar_panel(
        &self,
        canal_id: Id<ChannelMarker>,
        payload: &PayloadRespuesta,
    ) -> Result<Id<MessageMarker>, ErrorRespuesta> {
        let mut req = self.http.create_message(canal_id);
        if !payload.components.is_empty() {
            req = req.components(&payload.components);
        }
        if let Some(flags) = payload.flags {
            req = req.flags(flags);
        }
        if let Some(am) = &payload.allowed_mentions {
            req = req.allowed_mentions(Some(am));
        }
        let msg = req.await?.model().await?;
        Ok(msg.id)
    }

    async fn editar_panel(
        &self,
        canal_id: Id<ChannelMarker>,
        mensaje_id: Id<MessageMarker>,
        payload: &PayloadRespuesta,
    ) -> Result<(), ErrorRespuesta> {
        let mut req = self.http.update_message(canal_id, mensaje_id);
        if !payload.components.is_empty() {
            req = req.components(Some(&payload.components));
        }
        if let Some(flags) = payload.flags {
            req = req.flags(flags);
        }
        if let Some(am) = &payload.allowed_mentions {
            req = req.allowed_mentions(Some(am));
        }
        req.await?;
        Ok(())
    }

    async fn borrar_mensaje(
        &self,
        canal_id: Id<ChannelMarker>,
        mensaje_id: Id<MessageMarker>,
    ) -> Result<(), ErrorRespuesta> {
        self.http.delete_message(canal_id, mensaje_id).await?;
        Ok(())
    }
}

pub struct ClientePanelNulo;

#[async_trait::async_trait]
impl ClientePanel for ClientePanelNulo {
    async fn obtener_ultimo_mensaje_id(
        &self,
        _c: Id<ChannelMarker>,
    ) -> Result<Option<Id<MessageMarker>>, ErrorRespuesta> {
        Ok(None)
    }
    async fn enviar_panel(
        &self,
        _c: Id<ChannelMarker>,
        _p: &PayloadRespuesta,
    ) -> Result<Id<MessageMarker>, ErrorRespuesta> {
        Ok(Id::new(1))
    }
    async fn editar_panel(
        &self,
        _c: Id<ChannelMarker>,
        _m: Id<MessageMarker>,
        _p: &PayloadRespuesta,
    ) -> Result<(), ErrorRespuesta> {
        Ok(())
    }
    async fn borrar_mensaje(
        &self,
        _c: Id<ChannelMarker>,
        _m: Id<MessageMarker>,
    ) -> Result<(), ErrorRespuesta> {
        Ok(())
    }
}
