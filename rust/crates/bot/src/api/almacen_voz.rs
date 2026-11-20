//! Almacenes en memoria para ajustes y logs de voz de la API (Spec S-27 y S-30).

use crate::datos::GestorAjustesVoz;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct AlmacenAjustesVoz {
    ajustes: Mutex<HashMap<String, Value>>,
    gestor: Option<Arc<GestorAjustesVoz>>,
}

impl Default for AlmacenAjustesVoz {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl AlmacenAjustesVoz {
    pub fn nuevo() -> Self {
        Self {
            ajustes: Mutex::new(HashMap::new()),
            gestor: None,
        }
    }

    pub fn con_gestor(gestor: Arc<GestorAjustesVoz>) -> Self {
        Self {
            ajustes: Mutex::new(HashMap::new()),
            gestor: Some(gestor),
        }
    }

    pub fn ajustes_por_defecto() -> Value {
        json!({
            "enabled": false, "wake_word": "ignite", "language": "es",
            "tts_feedback_enabled": false, "tts_voice": "es-MX-DaliaNeural",
            "tts_pitch": 1.0, "tts_rate": 1.0,
            "tts_templates": crate::datos::ajustes_voz::plantillas_por_defecto(),
            "bindings": []
        })
    }

    pub fn obtener(&self, guild_id: &str) -> Value {
        self.ajustes
            .lock()
            .unwrap()
            .entry(guild_id.to_string())
            .or_insert_with(Self::ajustes_por_defecto)
            .clone()
    }

    pub async fn obtener_asincrono(&self, guild_id: &str) -> Value {
        if let Some(ref g) = self.gestor {
            let res = g.get_guild_settings(guild_id).await;
            serde_json::to_value(res).unwrap_or_else(|_| Self::ajustes_por_defecto())
        } else {
            self.obtener(guild_id)
        }
    }

    pub(crate) async fn obtener_para_voz(&self, guild_id: &str) -> crate::datos::AjustesVozGuild {
        if let Some(gestor) = &self.gestor {
            return gestor.get_guild_settings(guild_id).await;
        }
        serde_json::from_value(self.obtener(guild_id))
            .unwrap_or_else(|_| crate::datos::AjustesVozGuild::defecto(false, "ignite", false))
    }

    pub fn actualizar(&self, guild_id: &str, nuevo: Value) -> Value {
        let mut map = self.ajustes.lock().unwrap();
        let actual = map
            .entry(guild_id.to_string())
            .or_insert_with(Self::ajustes_por_defecto);
        if let (Some(obj_actual), Some(obj_nuevo)) = (actual.as_object_mut(), nuevo.as_object()) {
            for (k, v) in obj_nuevo {
                obj_actual.insert(k.clone(), v.clone());
            }
        }
        actual.clone()
    }

    pub async fn actualizar_asincrono(
        &self,
        guild_id: &str,
        nuevo: Value,
    ) -> Result<Value, (u16, &'static str, String)> {
        if let Some(ref g) = self.gestor {
            let res = g.update_guild_settings(guild_id, &nuevo).await?;
            Ok(serde_json::to_value(res).unwrap_or_else(|_| Self::ajustes_por_defecto()))
        } else {
            Ok(self.actualizar(guild_id, nuevo))
        }
    }

    pub fn reiniciar(&self, guild_id: &str) -> Value {
        let def = Self::ajustes_por_defecto();
        self.ajustes
            .lock()
            .unwrap()
            .insert(guild_id.to_string(), def.clone());
        def
    }

    pub async fn reiniciar_asincrono(&self, guild_id: &str) -> Value {
        if let Some(ref g) = self.gestor {
            let res = g.reset_guild_settings(guild_id).await;
            serde_json::to_value(res).unwrap_or_else(|_| Self::ajustes_por_defecto())
        } else {
            self.reiniciar(guild_id)
        }
    }
}

pub struct AlmacenLogsVoz {
    logs: Mutex<HashMap<String, Vec<Value>>>,
    almacen_real: Option<Arc<voice::AlmacenRegistroVoz>>,
}

impl Default for AlmacenLogsVoz {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl AlmacenLogsVoz {
    pub fn nuevo() -> Self {
        Self {
            logs: Mutex::new(HashMap::new()),
            almacen_real: None,
        }
    }

    pub fn con_almacen_real(almacen_real: Arc<voice::AlmacenRegistroVoz>) -> Self {
        Self {
            logs: Mutex::new(HashMap::new()),
            almacen_real: Some(almacen_real),
        }
    }

    pub fn obtener_logs(&self, guild_id: &str, limit: usize) -> Vec<Value> {
        let n = limit.clamp(1, 100);
        if let Some(ref real) = self.almacen_real {
            let entries = real.obtener(guild_id, Some(n));
            return entries
                .into_iter()
                .filter_map(|e| serde_json::to_value(e).ok())
                .collect();
        }
        let map = self.logs.lock().unwrap();
        let list = map.get(guild_id).cloned().unwrap_or_default();
        let start = list.len().saturating_sub(n);
        let mut res = list[start..].to_vec();
        res.reverse();
        res
    }

    pub fn limpiar(&self, guild_id: &str) {
        if let Some(ref real) = self.almacen_real {
            real.limpiar(guild_id);
        }
        self.logs.lock().unwrap().remove(guild_id);
    }
}
