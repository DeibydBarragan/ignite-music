//! Gestión de ajustes de voz y comandos personalizados con Supabase (Spec S-30).

use super::ajustes_voz_modelos::*;
use super::supabase::ClienteSupabaseRest;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::warn;

pub use super::ajustes_voz_modelos::{
    extraer_plantillas_limpias, plantillas_por_defecto, sanitize_wake_word, AjustesVozGuild,
    BindingVoz,
};

pub const CACHE_TTL: Duration = Duration::from_secs(60);

pub struct GestorAjustesVoz {
    cliente: Option<Arc<ClienteSupabaseRest>>,
    cache: RwLock<HashMap<String, (AjustesVozGuild, Instant)>>,
    pub default_enabled: bool,
    pub default_wake_word: String,
    pub default_feedback: bool,
}

impl GestorAjustesVoz {
    pub fn nuevo(
        cliente: Option<Arc<ClienteSupabaseRest>>,
        default_enabled: bool,
        default_wake_word: String,
        default_feedback: bool,
    ) -> Self {
        Self {
            cliente,
            cache: RwLock::new(HashMap::new()),
            default_enabled,
            default_wake_word: sanitize_wake_word(&default_wake_word),
            default_feedback,
        }
    }

    pub fn obtener_defecto(&self) -> AjustesVozGuild {
        AjustesVozGuild::defecto(
            self.default_enabled,
            &self.default_wake_word,
            self.default_feedback,
        )
    }

    pub async fn get_guild_settings(&self, guild_id: &str) -> AjustesVozGuild {
        if guild_id.is_empty() {
            return self.obtener_defecto();
        }
        if let Some((d, exp)) = self.cache.read().unwrap().get(guild_id) {
            if Instant::now() < *exp {
                return d.clone();
            }
        }
        let cliente = match &self.cliente {
            Some(c) => c,
            None => return self.obtener_defecto(),
        };

        let def = self.obtener_defecto();
        let r_sett = format!("rest/v1/guild_voice_settings?guild_id=eq.{guild_id}&select=*");
        let r_bind =
            format!("rest/v1/guild_voice_bindings?guild_id=eq.{guild_id}&select=phrase,command_id");

        let (res_sett, res_bind) =
            tokio::join!(cliente.get(&r_sett).send(), cliente.get(&r_bind).send());
        let raw_sett: Option<Value> = match res_sett {
            Ok(r) if r.status().is_success() => {
                r.json::<Vec<Value>>().await.ok().and_then(|mut v| v.pop())
            }
            _ => None,
        };
        let bindings: Vec<BindingVoz> = match res_bind {
            Ok(r) if r.status().is_success() => {
                r.json::<Vec<BindingVoz>>().await.unwrap_or_default()
            }
            _ => Vec::new(),
        };

        let raw_tmpl = raw_sett
            .as_ref()
            .and_then(|s| s.get("tts_templates"))
            .cloned()
            .unwrap_or(json!({}));

        let res = construir_ajustes_guild(raw_sett.as_ref(), &raw_tmpl, bindings, &def);
        self.cache.write().unwrap().insert(
            guild_id.to_string(),
            (res.clone(), Instant::now() + CACHE_TTL),
        );
        res
    }

    pub async fn update_guild_settings(
        &self,
        guild_id: &str,
        payload: &Value,
    ) -> Result<AjustesVozGuild, (u16, &'static str, String)> {
        let actual = self.get_guild_settings(guild_id).await;
        let mut clean_tmpl = actual.tts_templates.clone();
        if let Some(pt) = payload.get("tts_templates") {
            clean_tmpl.extend(extraer_plantillas_limpias(pt));
        }

        let tts_voice = payload
            .get("tts_voice")
            .and_then(Value::as_str)
            .unwrap_or(&actual.tts_voice)
            .to_string();
        let tts_pitch = payload
            .get("tts_pitch")
            .and_then(Value::as_f64)
            .unwrap_or(actual.tts_pitch);
        let tts_rate = payload
            .get("tts_rate")
            .and_then(Value::as_f64)
            .unwrap_or(actual.tts_rate);
        let bindings = payload
            .get("bindings")
            .and_then(Value::as_array)
            .map(|arr| {
                serde_json::from_value(Value::Array(arr.clone()))
                    .unwrap_or_else(|_| actual.bindings.clone())
            })
            .unwrap_or(actual.bindings);

        let updated = AjustesVozGuild {
            csv_enabled: actual.csv_enabled,
            enabled: payload
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(actual.enabled),
            wake_word: payload
                .get("wake_word")
                .and_then(Value::as_str)
                .map(sanitize_wake_word)
                .unwrap_or(actual.wake_word),
            language: payload
                .get("language")
                .and_then(Value::as_str)
                .unwrap_or(&actual.language)
                .into(),
            tts_voice: tts_voice.clone(),
            tts_pitch,
            tts_rate,
            tts_feedback_enabled: payload
                .get("tts_feedback_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(actual.tts_feedback_enabled),
            tts_templates: clean_tmpl.clone(),
            bindings,
        };

        if let Some(cliente) = &self.cliente {
            let mut db_tmpl = json!(clean_tmpl);
            if let Some(map) = db_tmpl.as_object_mut() {
                map.insert("_voice".into(), json!(tts_voice));
                map.insert("_pitch".into(), json!(tts_pitch));
                map.insert("_rate".into(), json!(tts_rate));
                map.insert("_csv".into(), json!(updated.csv_enabled));
            }
            let row = json!({
                "guild_id": guild_id, "enabled": updated.enabled, "wake_word": updated.wake_word,
                "language": updated.language, "tts_feedback_enabled": updated.tts_feedback_enabled,
                "tts_templates": db_tmpl, "updated_at": chrono::Utc::now().to_rfc3339()
            });

            let res = match cliente.post("rest/v1/guild_voice_settings") {
                Ok(req) => {
                    req.header("Prefer", "resolution=merge-duplicates")
                        .json(&row)
                        .send()
                        .await
                }
                Err(e) => return Err((503, "DB_UNAVAILABLE", e.to_string())),
            };
            match res {
                Ok(r) if r.status().is_success() => {
                    if let Some(b_arr) = payload.get("bindings").and_then(Value::as_array) {
                        if let Ok(req) = cliente.delete(&format!(
                            "rest/v1/guild_voice_bindings?guild_id=eq.{guild_id}"
                        )) {
                            let _ = req.send().await;
                        }
                        if !b_arr.is_empty() {
                            let rows: Vec<Value> = updated
                                .bindings
                                .iter()
                                .map(|b| {
                                    json!({
                                        "guild_id": guild_id, "phrase": b.phrase.trim().to_lowercase(),
                                        "command_id": b.command_id.trim()
                                    })
                                })
                                .collect();
                            if let Ok(req) = cliente.post("rest/v1/guild_voice_bindings") {
                                let _ = req.json(&rows).send().await;
                            }
                        }
                    }
                }
                Ok(r) => {
                    return Err((
                        503,
                        "DB_UNAVAILABLE",
                        format!("DB HTTP error: {}", r.status()),
                    ))
                }
                Err(e) => {
                    return Err((503, "DB_UNAVAILABLE", format!("Database unavailable: {e}")))
                }
            }
        }
        self.cache.write().unwrap().insert(
            guild_id.to_string(),
            (updated.clone(), Instant::now() + CACHE_TTL),
        );
        Ok(updated)
    }

    pub async fn reset_guild_settings(&self, guild_id: &str) -> AjustesVozGuild {
        if let Some(c) = &self.cliente {
            for ruta in [
                format!("rest/v1/guild_voice_bindings?guild_id=eq.{guild_id}"),
                format!("rest/v1/guild_voice_settings?guild_id=eq.{guild_id}"),
            ] {
                match c.delete(&ruta) {
                    Ok(req) => match req.send().await {
                        Ok(r) if r.status().is_success() => {}
                        Ok(r) => warn!("reinicio de ajustes de voz: DELETE HTTP {}", r.status()),
                        Err(e) => warn!("reinicio de ajustes de voz: DELETE fallido: {e}"),
                    },
                    Err(e) => warn!("reinicio de ajustes de voz sin borrar: {e}"),
                }
            }
        }
        self.cache.write().unwrap().remove(guild_id);
        self.obtener_defecto()
    }

    pub fn limpiar_cache(&self) {
        self.cache.write().unwrap().clear();
    }
}
