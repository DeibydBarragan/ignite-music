//! Pruebas para gestión de respuestas y emisor de interacciones (Spec S-23).

use bot::respuesta::{
    avisar, diferir, responder, EmisorRespuesta, ErrorRespuesta, EstadoInteraccion,
    PayloadRespuesta, TipoEnvioRespuesta, ERROR_INTERACCION_DESCONOCIDA, ERROR_YA_RECONOCIDA,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use twilight_model::channel::message::MessageFlags;
use twilight_model::http::interaction::InteractionResponse;
use twilight_model::id::marker::{ApplicationMarker, InteractionMarker};
use twilight_model::id::Id;

struct MockEmisor {
    pub respuestas: AtomicUsize,
    pub actualizaciones: AtomicUsize,
    pub seguimientos: AtomicUsize,
    pub flags_actualizados: Mutex<Option<MessageFlags>>,
    pub enviada: Mutex<Option<InteractionResponse>>,
    pub error_retorno: Mutex<Option<ErrorRespuesta>>,
}

impl MockEmisor {
    fn nuevo() -> Self {
        Self {
            respuestas: AtomicUsize::new(0),
            actualizaciones: AtomicUsize::new(0),
            seguimientos: AtomicUsize::new(0),
            flags_actualizados: Mutex::new(None),
            enviada: Mutex::new(None),
            error_retorno: Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl EmisorRespuesta for MockEmisor {
    async fn enviar_respuesta(
        &self,
        _: Id<ApplicationMarker>,
        _: Id<InteractionMarker>,
        _: &str,
        respuesta: &InteractionResponse,
    ) -> Result<(), ErrorRespuesta> {
        self.respuestas.fetch_add(1, Ordering::SeqCst);
        *self.enviada.lock().unwrap() = Some(respuesta.clone());
        if let Some(e) = self.error_retorno.lock().unwrap().take() {
            Err(e)
        } else {
            Ok(())
        }
    }

    async fn actualizar_respuesta(
        &self,
        _: Id<ApplicationMarker>,
        _: &str,
        payload: &PayloadRespuesta,
    ) -> Result<(), ErrorRespuesta> {
        self.actualizaciones.fetch_add(1, Ordering::SeqCst);
        *self.flags_actualizados.lock().unwrap() = payload.flags;
        if let Some(e) = self.error_retorno.lock().unwrap().take() {
            Err(e)
        } else {
            Ok(())
        }
    }

    async fn enviar_seguimiento(
        &self,
        _: Id<ApplicationMarker>,
        _: &str,
        _: &PayloadRespuesta,
    ) -> Result<(), ErrorRespuesta> {
        self.seguimientos.fetch_add(1, Ordering::SeqCst);
        if let Some(e) = self.error_retorno.lock().unwrap().take() {
            Err(e)
        } else {
            Ok(())
        }
    }
}

#[tokio::test]
async fn diferir_reconoce_antes_de_editar_el_aviso() {
    use twilight_model::http::interaction::InteractionResponseType;
    let mock = MockEmisor::nuevo();
    let mut estado = EstadoInteraccion::nuevo(Id::new(1), Id::new(2), "token".into());
    *mock.error_retorno.lock().unwrap() = Some(ErrorRespuesta::CodigoDiscord(10062));
    assert!(diferir(&mock, &mut estado).await.is_err());
    assert!(!estado.diferida);
    diferir(&mock, &mut estado).await.unwrap();
    let enviada = mock.enviada.lock().unwrap().take().unwrap();
    assert_eq!(
        enviada.kind,
        InteractionResponseType::DeferredChannelMessageWithSource
    );
    assert_eq!(enviada.data.unwrap().flags, Some(MessageFlags::EPHEMERAL));
    assert_eq!(
        avisar(&mock, &estado, "Prueba", true).await.unwrap(),
        Some(TipoEnvioRespuesta::EdicionDiferida)
    );
    assert_eq!(*mock.flags_actualizados.lock().unwrap(), None);
}

#[tokio::test]
async fn test_emisor_respuesta_transiciones() {
    let mock = MockEmisor::nuevo();
    let mut st = EstadoInteraccion::nuevo(Id::new(1), Id::new(2), "token".into());
    let p = PayloadRespuesta {
        content: Some("Hola".into()),
        components: vec![],
        flags: Some(MessageFlags::EPHEMERAL | MessageFlags::IS_COMPONENTS_V2),
        allowed_mentions: None,
    };

    // 1. Sin respuesta previa: RespuestaDirecta
    assert_eq!(
        responder(&mock, &st, &p).await.unwrap(),
        Some(TipoEnvioRespuesta::RespuestaDirecta)
    );
    assert_eq!(mock.respuestas.load(Ordering::SeqCst), 1);

    // 2. Diferida y no respondida: EdicionDiferida conservando solo IS_COMPONENTS_V2
    st.diferida = true;
    assert_eq!(
        responder(&mock, &st, &p).await.unwrap(),
        Some(TipoEnvioRespuesta::EdicionDiferida)
    );
    assert_eq!(mock.actualizaciones.load(Ordering::SeqCst), 1);
    assert_eq!(
        *mock.flags_actualizados.lock().unwrap(),
        Some(MessageFlags::IS_COMPONENTS_V2)
    );

    // 3. Ya respondida: Seguimiento
    st.respondida = true;
    assert_eq!(
        responder(&mock, &st, &p).await.unwrap(),
        Some(TipoEnvioRespuesta::Seguimiento)
    );
    assert_eq!(mock.seguimientos.load(Ordering::SeqCst), 1);

    // 4. Códigos ignorados: 10062 y 40060 retornan Ok(None)
    st.diferida = false;
    st.respondida = false;
    *mock.error_retorno.lock().unwrap() =
        Some(ErrorRespuesta::CodigoDiscord(ERROR_INTERACCION_DESCONOCIDA));
    assert_eq!(responder(&mock, &st, &p).await.unwrap(), None);

    *mock.error_retorno.lock().unwrap() = Some(ErrorRespuesta::CodigoDiscord(ERROR_YA_RECONOCIDA));
    assert_eq!(responder(&mock, &st, &p).await.unwrap(), None);

    // 5. Código no ignorado: 50013 sube como error
    *mock.error_retorno.lock().unwrap() = Some(ErrorRespuesta::CodigoDiscord(50013));
    assert_eq!(
        responder(&mock, &st, &p).await.unwrap_err(),
        ErrorRespuesta::CodigoDiscord(50013)
    );

    // 6. avisar crea aviso efímero
    assert_eq!(
        avisar(&mock, &st, "Aviso", true).await.unwrap(),
        Some(TipoEnvioRespuesta::RespuestaDirecta)
    );
}
