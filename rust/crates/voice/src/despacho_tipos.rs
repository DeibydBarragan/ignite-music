use std::collections::HashMap;

use crate::matcher::FrasePersonalizada;

#[derive(Debug, Clone)]
pub struct SolicitudDespacho {
    pub guild_id: String,
    pub user_id: String,
    pub text: String,
    pub channel_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AjustesVoz {
    pub enabled: bool,
    pub wake_word: Option<String>,
    pub bindings: Option<Vec<FrasePersonalizada>>,
    pub tts_feedback_enabled: bool,
    pub tts_templates: Option<HashMap<String, String>>,
    pub tts_voice: Option<String>,
    pub tts_rate: Option<f64>,
    pub tts_pitch: Option<f64>,
    pub sound_api_url: Option<String>,
    pub sound_api_secret: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResultadoDespacho {
    pub ejecutado: bool,
    pub comando: Option<String>,
    pub consulta: Option<String>,
    pub motivo: Option<String>,
    pub mensaje_tts: Option<String>,
}

impl ResultadoDespacho {
    pub fn fail(motivo: &str, comando: Option<&str>, consulta: Option<&str>) -> Self {
        Self {
            ejecutado: false,
            comando: comando.map(Into::into),
            consulta: consulta.map(Into::into),
            motivo: Some(motivo.into()),
            mensaje_tts: None,
        }
    }

    pub fn ok(comando: &str, consulta: &str, mensaje_tts: Option<String>) -> Self {
        Self {
            ejecutado: true,
            comando: Some(comando.into()),
            consulta: if consulta.is_empty() {
                None
            } else {
                Some(consulta.into())
            },
            motivo: None,
            mensaje_tts,
        }
    }
}

pub fn resolver_mensaje_feedback(command_id: &str, query: &str) -> Option<String> {
    resolver_mensaje_feedback_con_plantillas(command_id, query, None)
}

pub fn resolver_mensaje_feedback_con_plantillas(
    command_id: &str,
    query: &str,
    custom: Option<&HashMap<String, String>>,
) -> Option<String> {
    let mut params = HashMap::new();
    if !query.is_empty() {
        params.insert("query".to_string(), query.to_string());
        params.insert("title".to_string(), query.to_string());
    }
    crate::tts::resolve_feedback_message(command_id, &params, custom)
}

pub fn disparar_tts(
    solicitud: &SolicitudDespacho,
    channel_id: Option<&str>,
    command_id: &str,
    query: &str,
    ajustes: Option<&AjustesVoz>,
    forzar_enabled: bool,
) {
    let Some(cid) = channel_id else { return };
    let is_enabled = forzar_enabled || ajustes.map(|a| a.tts_feedback_enabled).unwrap_or(false);
    if !is_enabled {
        return;
    }
    let Some(api_url) = ajustes
        .and_then(|a| a.sound_api_url.clone())
        .filter(|u| !u.is_empty())
    else {
        return;
    };
    let mut params = HashMap::new();
    if !query.is_empty() {
        params.insert("query".to_string(), query.to_string());
        params.insert("title".to_string(), query.to_string());
    }
    let aviso = crate::tts::AvisoTts {
        guild_id: solicitud.guild_id.clone(),
        channel_id: cid.to_string(),
        user_id: Some(solicitud.user_id.clone()),
        command_id: command_id.to_string(),
        params,
    };
    let opts = crate::tts::OpcionesTts {
        enabled: Some(true),
        templates: ajustes.and_then(|a| a.tts_templates.clone()),
        voice: ajustes.and_then(|a| a.tts_voice.clone()),
        rate: ajustes.and_then(|a| a.tts_rate),
        pitch: ajustes.and_then(|a| a.tts_pitch),
        api_url: Some(api_url),
        api_secret: ajustes.and_then(|a| a.sound_api_secret.clone()),
        ..Default::default()
    };
    tokio::spawn(async move {
        let _ = crate::tts::send_tts_feedback(&aviso, Some(&opts)).await;
    });
}
