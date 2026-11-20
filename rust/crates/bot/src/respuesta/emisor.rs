use super::PayloadRespuesta;
use std::sync::Arc;
use twilight_http::Client as HttpClient;
use twilight_model::http::interaction::InteractionResponse;
use twilight_model::id::marker::{ApplicationMarker, InteractionMarker};
use twilight_model::id::Id;

pub const ERROR_INTERACCION_DESCONOCIDA: u64 = 10062;
pub const ERROR_YA_RECONOCIDA: u64 = 40060;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorRespuesta {
    CodigoDiscord(u64),
    Http(String),
}

impl ErrorRespuesta {
    pub fn es_ignorado(&self) -> bool {
        matches!(
            self,
            Self::CodigoDiscord(ERROR_INTERACCION_DESCONOCIDA | ERROR_YA_RECONOCIDA)
        )
    }
}

impl std::fmt::Display for ErrorRespuesta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CodigoDiscord(code) => write!(f, "Discord API error code {code}"),
            Self::Http(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ErrorRespuesta {}

impl From<twilight_http::Error> for ErrorRespuesta {
    fn from(err: twilight_http::Error) -> Self {
        if let twilight_http::error::ErrorType::Response {
            error: twilight_http::api_error::ApiError::General(gen_err),
            ..
        } = err.kind()
        {
            return Self::CodigoDiscord(gen_err.code);
        }
        Self::Http(err.to_string())
    }
}

impl From<twilight_http::response::DeserializeBodyError> for ErrorRespuesta {
    fn from(err: twilight_http::response::DeserializeBodyError) -> Self {
        Self::Http(err.to_string())
    }
}

#[async_trait::async_trait]
pub trait EmisorRespuesta: Send + Sync {
    async fn enviar_respuesta(
        &self,
        application_id: Id<ApplicationMarker>,
        interaction_id: Id<InteractionMarker>,
        token: &str,
        response: &InteractionResponse,
    ) -> Result<(), ErrorRespuesta>;

    async fn actualizar_respuesta(
        &self,
        application_id: Id<ApplicationMarker>,
        token: &str,
        payload: &PayloadRespuesta,
    ) -> Result<(), ErrorRespuesta>;

    async fn enviar_seguimiento(
        &self,
        application_id: Id<ApplicationMarker>,
        token: &str,
        payload: &PayloadRespuesta,
    ) -> Result<(), ErrorRespuesta>;
}

pub struct EmisorRespuestaReal {
    pub http: Arc<HttpClient>,
}

impl EmisorRespuestaReal {
    pub fn nuevo(http: Arc<HttpClient>) -> Self {
        Self { http }
    }
}

#[async_trait::async_trait]
impl EmisorRespuesta for EmisorRespuestaReal {
    async fn enviar_respuesta(
        &self,
        application_id: Id<ApplicationMarker>,
        interaction_id: Id<InteractionMarker>,
        token: &str,
        response: &InteractionResponse,
    ) -> Result<(), ErrorRespuesta> {
        self.http
            .interaction(application_id)
            .create_response(interaction_id, token, response)
            .await?;
        Ok(())
    }

    async fn actualizar_respuesta(
        &self,
        application_id: Id<ApplicationMarker>,
        token: &str,
        payload: &PayloadRespuesta,
    ) -> Result<(), ErrorRespuesta> {
        let client = self.http.interaction(application_id);
        let mut req = client.update_response(token);
        if let Some(content) = payload.content.as_deref() {
            req = req.content(Some(content));
        }
        if !payload.components.is_empty() {
            req = req.components(Some(&payload.components));
        }
        if let Some(flags) = payload.flags {
            req = req.flags(flags);
        }
        req.await?;
        Ok(())
    }

    async fn enviar_seguimiento(
        &self,
        application_id: Id<ApplicationMarker>,
        token: &str,
        payload: &PayloadRespuesta,
    ) -> Result<(), ErrorRespuesta> {
        let client = self.http.interaction(application_id);
        let content = payload.content.as_deref().unwrap_or_default();
        let mut req = client.create_followup(token).content(content);
        if !payload.components.is_empty() {
            req = req.components(&payload.components);
        }
        if let Some(flags) = payload.flags {
            req = req.flags(flags);
        }
        req.await?;
        Ok(())
    }
}
