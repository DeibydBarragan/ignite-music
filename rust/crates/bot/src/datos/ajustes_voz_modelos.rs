//! Modelos y utilidades de ajustes de voz (Spec S-30).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub fn sanitize_wake_word(raw: &str) -> String {
    if raw.trim().is_empty() {
        return "ignite".to_string();
    }
    let tokens: Vec<String> = raw
        .split(',')
        .map(|w| w.trim().to_lowercase())
        .filter(|w| !w.is_empty() && w != "bot")
        .collect();
    if tokens.is_empty() {
        "ignite".into()
    } else {
        tokens.join(", ")
    }
}

pub fn plantillas_por_defecto() -> HashMap<String, String> {
    const DEFS: &[(&str, &str)] = &[
        ("play", "Reproduciendo {query}"),
        ("play_next", "Añadida {query} a continuación"),
        ("enqueue", "Añadida {query} a la lista"),
        ("skip", "Saltando canción"),
        ("pause", "Música pausada"),
        ("resume", "Reanudando música"),
        ("stop", "Reproducción detenida"),
        ("previous", "Canción anterior"),
        ("shuffle", "Lista en modo aleatorio"),
        ("autoplay_on", "Autoplay activado"),
        ("autoplay_off", "Autoplay desactivado"),
        ("volume", "Volumen ajustado al {query}"),
        ("queue", "Mostrando la lista"),
        ("hola_que_hace", "Hola que hace"),
    ];
    DEFS.iter()
        .map(|(k, v)| ((*k).into(), (*v).into()))
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BindingVoz {
    pub phrase: String,
    pub command_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AjustesVozGuild {
    #[serde(skip)]
    pub csv_enabled: bool,
    pub enabled: bool,
    pub wake_word: String,
    pub language: String,
    pub tts_voice: String,
    pub tts_pitch: f64,
    pub tts_rate: f64,
    pub tts_feedback_enabled: bool,
    pub tts_templates: HashMap<String, String>,
    pub bindings: Vec<BindingVoz>,
}

impl AjustesVozGuild {
    pub fn defecto(enabled: bool, wake_word: &str, feedback: bool) -> Self {
        Self {
            csv_enabled: false,
            enabled,
            wake_word: wake_word.to_string(),
            language: "es".to_string(),
            tts_voice: "es-MX-DaliaNeural".to_string(),
            tts_pitch: 1.0,
            tts_rate: 1.0,
            tts_feedback_enabled: feedback,
            tts_templates: plantillas_por_defecto(),
            bindings: Vec::new(),
        }
    }
}

pub fn extraer_plantillas_limpias(raw: &Value) -> HashMap<String, String> {
    let mut clean = HashMap::new();
    if let Some(obj) = raw.as_object() {
        for (k, v) in obj {
            if !k.starts_with('_') {
                if let Some(s) = v.as_str() {
                    clean.insert(k.clone(), s.to_string());
                }
            }
        }
    }
    clean
}

pub fn construir_ajustes_guild(
    raw_sett: Option<&Value>,
    raw_tmpl: &Value,
    bindings: Vec<BindingVoz>,
    def: &AjustesVozGuild,
) -> AjustesVozGuild {
    let mut tts_templates = plantillas_por_defecto();
    tts_templates.extend(extraer_plantillas_limpias(raw_tmpl));

    AjustesVozGuild {
        csv_enabled: raw_tmpl
            .get("_csv")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        enabled: raw_sett
            .and_then(|s| s.get("enabled")?.as_bool())
            .unwrap_or(def.enabled),
        wake_word: raw_sett
            .and_then(|s| s.get("wake_word")?.as_str())
            .map(sanitize_wake_word)
            .unwrap_or_else(|| def.wake_word.clone()),
        language: raw_sett
            .and_then(|s| s.get("language")?.as_str())
            .unwrap_or("es")
            .into(),
        tts_voice: raw_tmpl
            .get("_voice")
            .and_then(|v| v.as_str())
            .unwrap_or(&def.tts_voice)
            .into(),
        tts_pitch: raw_tmpl
            .get("_pitch")
            .and_then(|v| v.as_f64())
            .unwrap_or(def.tts_pitch),
        tts_rate: raw_tmpl
            .get("_rate")
            .and_then(|v| v.as_f64())
            .unwrap_or(def.tts_rate),
        tts_feedback_enabled: raw_sett
            .and_then(|s| s.get("tts_feedback_enabled")?.as_bool())
            .unwrap_or(def.tts_feedback_enabled),
        tts_templates,
        bindings,
    }
}
