use crate::cola::{Cola, ElementoCola};
use medios::Cancion;
use nucleo::almacen::{AlmacenError, AlmacenJson};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

pub const MAX_AGE_MS: u64 = 30 * 60 * 1000;
pub const MAX_QUEUE_SIZE: usize = 500;
/// Volumen de una cola nueva. Igual que `config.bot.defaultVolume` de Node (100).
pub const DEFAULT_VOLUME: u8 = 100;

pub fn ahora_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct UploaderSesion {
    pub name: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct UsuarioSesion {
    pub id: String,
    pub username: String,
    pub avatar: Option<String>,
    pub bot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CancionSesion {
    pub plugin: Option<String>,
    pub source: Option<String>,
    pub play_from_source: bool,
    pub id: String,
    pub name: String,
    pub url: String,
    pub duration: u64,
    pub is_live: bool,
    pub thumbnail: Option<String>,
    pub uploader: Option<UploaderSesion>,
    pub age_restricted: bool,
    pub requester_id: Option<String>,
    pub requester_user: Option<UsuarioSesion>,
    pub autoplay: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SesionGuardada {
    pub voice_channel_id: Option<String>,
    pub text_channel_id: Option<String>,
    pub songs: Vec<CancionSesion>,
    pub position: u64,
    pub volume: u8,
    pub repeat_mode: u8,
    pub autoplay: bool,
    pub filters: Vec<String>,
    pub paused: bool,
    pub saved_at: u64,
}

#[derive(Clone)]
pub struct AlmacenSesiones {
    almacen: Arc<AlmacenJson>,
    plugins_conocidos: Arc<HashSet<String>>,
}

impl AlmacenSesiones {
    pub fn nuevo(almacen: AlmacenJson) -> Self {
        Self::nuevo_con_plugins(almacen, &["youtube", "spotify", "direct", "generic"])
    }

    pub fn nuevo_con_plugins(almacen: AlmacenJson, plugins: &[&str]) -> Self {
        let set: HashSet<String> = plugins.iter().map(|s| (*s).to_string()).collect();
        Self {
            almacen: Arc::new(almacen),
            plugins_conocidos: Arc::new(set),
        }
    }

    pub fn dir(&self) -> &Path {
        self.almacen.dir()
    }

    pub async fn initialize(&self) -> Result<(), AlmacenError> {
        self.almacen.initialize().await
    }

    pub fn plugin_conocido(&self, plugin: &str) -> bool {
        self.plugins_conocidos.contains(plugin)
    }

    pub fn serializar_cancion(&self, elem: &ElementoCola) -> Option<CancionSesion> {
        let plugin = self
            .plugin_conocido(&elem.cancion.fuente)
            .then(|| elem.cancion.fuente.clone());
        let is_bot = elem.requester_id.as_deref() == Some("bot");
        let uploader =
            (!elem.cancion.artista.is_empty() || elem.cancion.autor_url.is_some()).then(|| {
                UploaderSesion {
                    name: (!elem.cancion.artista.is_empty()).then(|| elem.cancion.artista.clone()),
                    url: elem.cancion.autor_url.clone(),
                }
            });
        let requester_user = elem.requester_id.as_ref().map(|id| UsuarioSesion {
            id: id.clone(),
            username: elem.requester_name.clone().unwrap_or_else(|| id.clone()),
            avatar: elem.requester_avatar.clone(),
            bot: is_bot,
        });

        Some(CancionSesion {
            plugin,
            source: Some(elem.cancion.fuente.clone()),
            play_from_source: elem.cancion.fuente == "youtube",
            id: elem.cancion.id.clone(),
            name: elem.cancion.nombre.clone(),
            url: elem.cancion.url.clone(),
            duration: elem.cancion.duracion,
            is_live: elem.cancion.es_directo,
            thumbnail: elem.cancion.miniatura.clone(),
            uploader,
            age_restricted: elem.cancion.edad_restringida,
            requester_id: elem.requester_id.clone(),
            requester_user,
            autoplay: is_bot,
        })
    }

    pub fn serializar(&self, cola: &Cola) -> SesionGuardada {
        let songs: Vec<CancionSesion> = cola
            .canciones
            .iter()
            .take(MAX_QUEUE_SIZE)
            .filter_map(|e| self.serializar_cancion(e))
            .filter(|s| s.plugin.is_some() && !s.url.is_empty())
            .collect();

        SesionGuardada {
            voice_channel_id: cola.canal_voz.clone(),
            text_channel_id: cola.canal_texto.clone(),
            songs,
            position: cola.posicion_segundos,
            volume: cola.volumen,
            repeat_mode: cola.modo_repeticion as u8,
            autoplay: cola.autoplay,
            filters: cola.filtros.clone(),
            paused: cola.pausada,
            saved_at: ahora_ms(),
        }
    }

    pub async fn guardar(&self, cola: &Cola) -> Result<bool, AlmacenError> {
        if cola.canciones.is_empty() {
            self.almacen.delete(&cola.guild_id).await?;
            return Ok(false);
        }
        let sesion = self.serializar(cola);
        let value = serde_json::to_value(&sesion)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.almacen.set(&cola.guild_id, value).await?;
        Ok(true)
    }

    pub fn cargar_con_tiempo(&self, guild_id: &str, ahora: u64) -> Option<SesionGuardada> {
        let val = self.almacen.get(guild_id).ok()??;
        let data: SesionGuardada = serde_json::from_value(val).ok()?;
        if data.songs.is_empty() {
            return None;
        }
        if ahora > 0 && ahora.saturating_sub(data.saved_at) > MAX_AGE_MS {
            return None;
        }
        Some(data)
    }

    pub fn cargar(&self, guild_id: &str) -> Option<SesionGuardada> {
        self.cargar_con_tiempo(guild_id, ahora_ms())
    }

    pub async fn eliminar(&self, guild_id: &str) -> Result<(), AlmacenError> {
        self.almacen.delete(guild_id).await
    }

    pub fn guild_ids(&self) -> Vec<String> {
        self.almacen.keys()
    }

    pub fn reconstruir_canciones(&self, sesion: &SesionGuardada) -> Vec<ElementoCola> {
        sesion
            .songs
            .iter()
            .filter(|s| {
                s.plugin
                    .as_deref()
                    .map(|p| self.plugin_conocido(p))
                    .unwrap_or(false)
            })
            .map(|s| {
                let fuente = s
                    .source
                    .clone()
                    .or_else(|| s.plugin.clone())
                    .unwrap_or_else(|| "youtube".to_string());
                let artista = s
                    .uploader
                    .as_ref()
                    .and_then(|u| u.name.clone())
                    .unwrap_or_default();
                let autor_url = s.uploader.as_ref().and_then(|u| u.url.clone());
                let cancion = Cancion {
                    fuente,
                    id: s.id.clone(),
                    nombre: s.name.clone(),
                    artista,
                    url: s.url.clone(),
                    stream_url: None,
                    es_directo: s.is_live,
                    duracion: s.duration,
                    miniatura: s.thumbnail.clone(),
                    autor_url,
                    edad_restringida: s.age_restricted,
                    es_letra: false,
                };
                let mut elem = ElementoCola::nuevo(cancion, s.requester_id.clone());
                if let Some(user) = &s.requester_user {
                    elem.requester_name = Some(user.username.clone());
                    elem.requester_avatar = user.avatar.clone();
                }
                elem
            })
            .collect()
    }

    pub async fn drain(&self) {
        self.almacen.drain().await;
    }
}
