//! Validación del cuerpo de PUT /voice/settings (Spec S-27). Separada de voz.rs por el límite de 250 líneas.

use axum::response::Response;
use serde_json::Value;

/// Devuelve la respuesta 400 si el cuerpo no es válido; `None` si pasa la validación.
pub(super) fn validar_cuerpo_ajustes(body: &Value) -> Option<Response> {
    let Some(fields) = body.as_object() else {
        return Some(crate::api::respuesta_problema(
            400,
            "INVALID_PARAMETER",
            "Expected an object",
        ));
    };
    for (name, value) in fields {
        let valid = match name.as_str() {
            "enabled" | "tts_feedback_enabled" => value.is_boolean(),
            "wake_word" | "tts_voice" | "language" => value.is_string(),
            "tts_pitch" | "tts_rate" => value.is_number(),
            "bindings" => value.is_array(),
            "tts_templates" => value.is_object(),
            _ => true,
        };
        if !valid {
            return Some(crate::api::respuesta_problema(
                400,
                "INVALID_PARAMETER",
                "Invalid voice setting type",
            ));
        }
    }
    let string_len = |v: &Value, min: usize, max: usize| {
        v.as_str().is_some_and(|s| {
            let len = s.trim().encode_utf16().count();
            (min..=max).contains(&len)
        })
    };
    let nested_valid = fields.iter().all(|(name, value)| match name.as_str() {
        "language" => string_len(value, 2, 10),
        "tts_voice" => string_len(value, 0, 100),
        "tts_templates" => value.as_object().is_some_and(|templates| {
            templates.values().all(|v| {
                v.is_number() || v.as_str().is_some_and(|s| s.encode_utf16().count() <= 200)
            })
        }),
        "bindings" => value.as_array().is_some_and(|bindings| {
            bindings.iter().all(|binding| {
                binding.get("phrase").is_some_and(|v| string_len(v, 1, 64))
                    && binding
                        .get("command_id")
                        .is_some_and(|v| string_len(v, 1, 32))
            })
        }),
        _ => true,
    });
    if !nested_valid {
        return Some(crate::api::respuesta_problema(
            400,
            "INVALID_PARAMETER",
            "Invalid voice settings",
        ));
    }
    if let Some(ww) = body.get("wake_word").and_then(|v| v.as_str()) {
        let t = ww.trim();
        if t.is_empty() || t.encode_utf16().count() > 256 {
            return Some(crate::api::respuesta_problema(
                400,
                "INVALID_PARAMETER",
                "wake_word length must be between 1 and 256",
            ));
        }
    }
    if let Some(pitch) = body.get("tts_pitch").and_then(|v| v.as_f64()) {
        if !(0.4..=2.5).contains(&pitch) {
            return Some(crate::api::respuesta_problema(
                400,
                "INVALID_PARAMETER",
                "tts_pitch must be between 0.4 and 2.5",
            ));
        }
    }
    if let Some(rate) = body.get("tts_rate").and_then(|v| v.as_f64()) {
        if !(0.4..=2.5).contains(&rate) {
            return Some(crate::api::respuesta_problema(
                400,
                "INVALID_PARAMETER",
                "tts_rate must be between 0.4 and 2.5",
            ));
        }
    }
    if let Some(b) = body.get("bindings").and_then(|v| v.as_array()) {
        if b.len() > 100 {
            return Some(crate::api::respuesta_problema(
                400,
                "INVALID_PARAMETER",
                "bindings cannot exceed 100 items",
            ));
        }
    }
    None
}
