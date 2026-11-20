//! Gestión de respuestas a interacciones de Discord (Spec S-23).

pub mod emisor;
pub use emisor::*;

use serde::{Deserialize, Serialize};
use twilight_model::channel::message::{AllowedMentions, Component, MessageFlags};
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use twilight_model::id::marker::{ApplicationMarker, InteractionMarker};
use twilight_model::id::Id;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PayloadRespuesta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<Component>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<MessageFlags>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_mentions: Option<AllowedMentions>,
}

pub fn como_actualizacion(mut payload: PayloadRespuesta) -> PayloadRespuesta {
    if let Some(flags) = payload.flags {
        payload.flags = Some(flags & !MessageFlags::EPHEMERAL);
    }
    payload
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstadoInteraccion {
    pub id: Id<InteractionMarker>,
    pub application_id: Id<ApplicationMarker>,
    pub token: String,
    pub diferida: bool,
    pub respondida: bool,
}

impl EstadoInteraccion {
    pub fn nuevo(
        id: Id<InteractionMarker>,
        application_id: Id<ApplicationMarker>,
        token: String,
    ) -> Self {
        Self {
            id,
            application_id,
            token,
            diferida: false,
            respondida: false,
        }
    }

    pub fn desde_interaccion(
        interaction: &twilight_model::application::interaction::Interaction,
    ) -> Self {
        Self::nuevo(
            interaction.id,
            interaction.application_id,
            interaction.token.clone(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoEnvioRespuesta {
    RespuestaDirecta,
    EdicionDiferida,
    Seguimiento,
}

pub async fn diferir<E: EmisorRespuesta + ?Sized>(
    emisor: &E,
    estado: &mut EstadoInteraccion,
) -> Result<(), ErrorRespuesta> {
    let respuesta = InteractionResponse {
        kind: InteractionResponseType::DeferredChannelMessageWithSource,
        data: Some(InteractionResponseData {
            flags: Some(MessageFlags::EPHEMERAL),
            ..Default::default()
        }),
    };
    emisor
        .enviar_respuesta(estado.application_id, estado.id, &estado.token, &respuesta)
        .await?;
    estado.diferida = true;
    Ok(())
}

pub async fn responder<E: EmisorRespuesta + ?Sized>(
    emisor: &E,
    estado: &EstadoInteraccion,
    payload: &PayloadRespuesta,
) -> Result<Option<TipoEnvioRespuesta>, ErrorRespuesta> {
    if estado.diferida && !estado.respondida {
        let mut edit_payload = payload.clone();
        edit_payload.flags = payload
            .flags
            .map(|f| f & MessageFlags::IS_COMPONENTS_V2)
            .filter(|f| !f.is_empty());
        match emisor
            .actualizar_respuesta(estado.application_id, &estado.token, &edit_payload)
            .await
        {
            Ok(()) => Ok(Some(TipoEnvioRespuesta::EdicionDiferida)),
            Err(e) if e.es_ignorado() => Ok(None),
            Err(e) => Err(e),
        }
    } else if estado.respondida {
        match emisor
            .enviar_seguimiento(estado.application_id, &estado.token, payload)
            .await
        {
            Ok(()) => Ok(Some(TipoEnvioRespuesta::Seguimiento)),
            Err(e) if e.es_ignorado() => Ok(None),
            Err(e) => Err(e),
        }
    } else {
        let resp = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(InteractionResponseData {
                content: payload.content.clone(),
                components: if payload.components.is_empty() {
                    None
                } else {
                    Some(payload.components.clone())
                },
                flags: payload.flags,
                allowed_mentions: payload.allowed_mentions.clone(),
                ..Default::default()
            }),
        };
        match emisor
            .enviar_respuesta(estado.application_id, estado.id, &estado.token, &resp)
            .await
        {
            Ok(()) => Ok(Some(TipoEnvioRespuesta::RespuestaDirecta)),
            Err(e) if e.es_ignorado() => Ok(None),
            Err(e) => Err(e),
        }
    }
}

pub fn crear_aviso(mensaje: &str, efimero: bool) -> PayloadRespuesta {
    let truncated: String = mensaje.chars().take(3900).collect();
    let flags = efimero.then_some(MessageFlags::EPHEMERAL);
    PayloadRespuesta {
        content: Some(truncated),
        components: Vec::new(),
        flags,
        allowed_mentions: None,
    }
}

pub async fn avisar<E: EmisorRespuesta + ?Sized>(
    emisor: &E,
    estado: &EstadoInteraccion,
    mensaje: &str,
    efimero: bool,
) -> Result<Option<TipoEnvioRespuesta>, ErrorRespuesta> {
    let payload = crear_aviso(mensaje, efimero);
    responder(emisor, estado, &payload).await
}
