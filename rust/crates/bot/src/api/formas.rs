//! Formas de datos y serialización para el dashboard de la API (Spec S-27).

use player::ElementoCola;
use serde::{Deserialize, Serialize};

pub const FALLBACK_COVER: &str =
    "https://images.unsplash.com/photo-1470225620780-dba8ba36b745?w=300&q=80";

pub fn format_duration(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsuarioSolicitante {
    pub id: String,
    pub username: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PistaFrontend {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album_art: String,
    pub cover_url: Option<String>,
    pub duration: u64,
    pub formatted_duration: String,
    pub source: String,
    pub url: String,
    pub category: String,
    pub is_live: bool,
    pub is_lyric: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_autoplay: Option<bool>,
    pub requested_by: Option<UsuarioSolicitante>,
}

pub fn serializar_elemento_cola(elem: &ElementoCola, autoplay: bool) -> PistaFrontend {
    let song = &elem.cancion;
    let dur = song.duracion;
    let cover = song.miniatura.clone();
    let album_art = cover.clone().unwrap_or_else(|| FALLBACK_COVER.to_string());
    let source = if song.fuente == "spotify" {
        "spotify".to_string()
    } else {
        "youtube".to_string()
    };
    let requested_by = elem.requester_id.as_ref().map(|uid| UsuarioSolicitante {
        id: uid.clone(),
        username: elem
            .requester_name
            .clone()
            .unwrap_or_else(|| "Unknown User".to_string()),
        avatar: elem.requester_avatar.clone(),
    });

    PistaFrontend {
        id: if !song.id.is_empty() {
            song.id.clone()
        } else {
            song.url.clone()
        },
        title: if !song.nombre.is_empty() {
            song.nombre.clone()
        } else {
            "Unknown Title".to_string()
        },
        artist: if !song.artista.is_empty() {
            song.artista.clone()
        } else {
            "Unknown Artist".to_string()
        },
        album_art,
        cover_url: cover,
        duration: dur,
        formatted_duration: if dur == 0 {
            "00:00".into()
        } else {
            format_duration(dur)
        },
        source,
        url: song.url.clone(),
        category: "hits".to_string(),
        is_live: song.es_directo,
        is_lyric: song.es_letra,
        track_id: Some(elem.track_id.clone()),
        is_autoplay: Some(autoplay || elem.requester_id.is_none()),
        requested_by,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistFrontend {
    pub id: String,
    pub title: String,
    pub name: String,
    pub uploader: String,
    pub author: String,
    pub thumbnail: String,
    pub cover: String,
    pub track_count: u64,
    pub url: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoPlaylistFrontend {
    pub success: bool,
    pub kind: String,
    pub name: String,
    pub source: String,
    pub count: usize,
    pub truncated: bool,
    pub tracks: Vec<medios::TrackSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanalVozInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EstadoReproductorFrontend {
    pub has_queue: bool,
    pub is_playing: bool,
    pub is_paused: bool,
    pub current_time: u64,
    pub duration: u64,
    pub volume: u8,
    pub repeat_mode: String,
    pub autoplay: bool,
    pub filter: Option<String>,
    pub voice_channel: Option<CanalVozInfo>,
    pub current_song: Option<PistaFrontend>,
    pub queue: Vec<PistaFrontend>,
    pub disconnect_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServidorFrontend {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub member_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecomendacionRespuesta {
    pub source: String,
    #[serde(rename = "basedOn")]
    pub based_on: Option<PistaFrontend>,
    pub songs: CancionesRecomendadas,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum CancionesRecomendadas {
    Ranking(Vec<crate::datos::CancionRanking>),
    Radio(Vec<PistaFrontend>),
}

impl Default for RecomendacionRespuesta {
    fn default() -> Self {
        Self {
            source: "empty".to_string(),
            based_on: None,
            songs: CancionesRecomendadas::Ranking(Vec::new()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MiembroCanalFrontend {
    pub id: String,
    pub username: String,
    pub avatar: Option<String>,
    pub bot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CanalVozFrontend {
    pub id: String,
    pub name: String,
    pub bitrate: u32,
    pub user_limit: u32,
    pub user_count: usize,
    pub bot_present: bool,
    pub members: Vec<MiembroCanalFrontend>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BotVoiceStatus {
    pub id: String,
    pub name: String,
    pub deaf: bool,
    pub self_deaf: bool,
    pub server_deaf: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserVoiceStatus {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EstadoVozRespuesta {
    pub bot: Option<BotVoiceStatus>,
    pub user: Option<UserVoiceStatus>,
    pub can_control: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InfoPlaylistPlay {
    pub name: Option<String>,
    pub count: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoReproducirApi {
    pub success: bool,
    pub started: bool,
    pub position: usize,
    pub song: Option<PistaFrontend>,
    pub playlist: Option<InfoPlaylistPlay>,
}
