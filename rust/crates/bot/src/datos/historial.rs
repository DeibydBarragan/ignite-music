//! Historial de reproducciones local y persistente en Supabase (Spec S-30).

use super::supabase::ClienteSupabaseRest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::debug;

pub const MAX_LOCAL_HISTORY: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegistroReproduccion {
    pub guild_id: String,
    pub song_id: String,
    pub title: String,
    pub artist: String,
    pub album_art: Option<String>,
    pub url: String,
    pub duration: u32,
    pub source: String,
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub played_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct RegistroCancionInput {
    pub id: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_art: Option<String>,
    pub url: Option<String>,
    pub duration: Option<u32>,
    pub source: Option<String>,
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub dev_mode: bool,
}

pub struct ExclusionesHistorial {
    fechas: RwLock<HashSet<String>>,
    ruta: PathBuf,
    ultima_modificacion: RwLock<Option<std::time::SystemTime>>,
}

impl ExclusionesHistorial {
    pub fn nuevo(ruta: PathBuf) -> Self {
        let s = Self {
            fechas: RwLock::new(HashSet::new()),
            ruta,
            ultima_modificacion: RwLock::new(None),
        };
        s.cargar();
        s
    }

    pub fn cargar(&self) -> HashSet<String> {
        let mut set = HashSet::new();
        if let Ok(contenido) = fs::read_to_string(&self.ruta) {
            if let Ok(v) = serde_json::from_str::<Value>(&contenido) {
                if let Some(arr) = v.get("dates").and_then(Value::as_array) {
                    for item in arr {
                        if let Some(d) = item.as_str() {
                            set.insert(d.trim().to_string());
                        }
                    }
                }
            }
        }
        let mtime = fs::metadata(&self.ruta)
            .ok()
            .and_then(|m| m.modified().ok());
        *self.ultima_modificacion.write().unwrap() = mtime;
        *self.fechas.write().unwrap() = set.clone();
        set
    }

    pub fn recargar_si_cambio(&self) {
        let mtime = fs::metadata(&self.ruta)
            .ok()
            .and_then(|m| m.modified().ok());
        let necesita = {
            let guard = self.ultima_modificacion.read().unwrap();
            *guard != mtime
        };
        if necesita {
            self.cargar();
        }
    }

    pub fn es_excluido(&self, played_at: &str) -> bool {
        self.recargar_si_cambio();
        if played_at.len() < 10 {
            return false;
        }
        self.fechas.read().unwrap().contains(&played_at[..10])
    }

    pub fn obtener_fechas(&self) -> Vec<String> {
        self.recargar_si_cambio();
        self.fechas.read().unwrap().iter().cloned().collect()
    }
}

pub struct GestorHistorial {
    cliente: Option<Arc<ClienteSupabaseRest>>,
    dir_historial: PathBuf,
    pub exclusiones: Arc<ExclusionesHistorial>,
}

impl GestorHistorial {
    pub fn nuevo(
        cliente: Option<Arc<ClienteSupabaseRest>>,
        data_dir: &Path,
        exclusiones: Arc<ExclusionesHistorial>,
    ) -> Self {
        let dir_historial = data_dir.join("history");
        let _ = fs::create_dir_all(&dir_historial);
        Self {
            cliente,
            dir_historial,
            exclusiones,
        }
    }

    pub fn ruta_servidor(&self, guild_id: &str) -> PathBuf {
        self.dir_historial.join(format!("{guild_id}.json"))
    }

    pub fn cargar_local(&self, guild_id: &str) -> Vec<RegistroReproduccion> {
        let p = self.ruta_servidor(guild_id);
        if let Ok(c) = fs::read_to_string(&p) {
            serde_json::from_str::<Vec<RegistroReproduccion>>(&c).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    pub fn guardar_local(&self, guild_id: &str, mut lista: Vec<RegistroReproduccion>) {
        lista.truncate(MAX_LOCAL_HISTORY);
        let p = self.ruta_servidor(guild_id);
        if let Ok(json) = serde_json::to_string(&lista) {
            let _ = fs::write(p, json);
        }
    }

    pub async fn record_play(
        &self,
        guild_id: &str,
        song: &RegistroCancionInput,
        user_dev_mode: bool,
    ) {
        if guild_id.is_empty() || user_dev_mode || song.dev_mode {
            debug!("Ignorando guardado en historial para guild {guild_id} (modo dev)");
            return;
        }

        let url = song.url.clone().unwrap_or_default();
        let song_id = song
            .id
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| url.clone());
        let registro = RegistroReproduccion {
            guild_id: guild_id.to_string(),
            song_id,
            title: song
                .title
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "Desconocido".into()),
            artist: song
                .artist
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "Desconocido".into()),
            album_art: song.album_art.clone(),
            url,
            duration: song.duration.unwrap_or(0),
            source: song
                .source
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "youtube".into()),
            user_id: song.user_id.clone(),
            username: song.username.clone(),
            played_at: chrono::Utc::now().to_rfc3339(),
        };

        // 1. Guardar en local JSON
        let mut local = self.cargar_local(guild_id);
        local.insert(0, registro.clone());
        self.guardar_local(guild_id, local);

        // 2. Insertar en Supabase (song_plays)
        if let Some(cliente) = &self.cliente {
            match cliente.post("rest/v1/song_plays") {
                Ok(req) => match req.json(&registro).send().await {
                    Err(e) => debug!("Supabase song_plays insert exception: {e}"),
                    Ok(resp) if !resp.status().is_success() => {
                        let err = resp.text().await.unwrap_or_default();
                        debug!("Supabase song_plays insert error: {err}");
                    }
                    Ok(_) => {}
                },
                Err(e) => debug!("Supabase song_plays insert skipped: {e}"),
            }
        }
    }
}
