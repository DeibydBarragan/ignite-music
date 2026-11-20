use medios::Cancion;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum ModoRepeticion {
    #[default]
    Apagado = 0,
    Cancion = 1,
    Cola = 2,
}

impl ModoRepeticion {
    pub fn siguiente(self) -> Self {
        match self {
            Self::Apagado => Self::Cancion,
            Self::Cancion => Self::Cola,
            Self::Cola => Self::Apagado,
        }
    }

    pub fn desde_u8(v: u8) -> Self {
        match v % 3 {
            1 => Self::Cancion,
            2 => Self::Cola,
            _ => Self::Apagado,
        }
    }

    pub fn desde_str(s: &str) -> Option<Self> {
        match s {
            "off" | "disabled" | "0" => Some(Self::Apagado),
            "track" | "song" | "1" => Some(Self::Cancion),
            "queue" | "all" | "2" => Some(Self::Cola),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElementoCola {
    pub cancion: Cancion,
    pub track_id: String,
    pub requester_id: Option<String>,
    pub requester_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requester_avatar: Option<String>,
    pub timing_requested_at: Option<u64>,
    pub timing_resolved_at: Option<u64>,
    pub cut_retries: u32,
    pub original_repeat_mode: Option<ModoRepeticion>,
}

impl ElementoCola {
    pub fn nuevo(cancion: Cancion, requester_id: Option<String>) -> Self {
        let rand_suffix: u32 = rand::random();
        let track_id = format!("track-{:x}", rand_suffix);
        Self {
            cancion,
            track_id,
            requester_id,
            requester_name: None,
            requester_avatar: None,
            timing_requested_at: None,
            timing_resolved_at: None,
            cut_retries: 0,
            original_repeat_mode: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Cola {
    pub guild_id: String,
    pub canciones: Vec<ElementoCola>,
    pub canciones_previas: Vec<ElementoCola>,
    pub pausada: bool,
    pub detenida: bool,
    pub volumen: u8,
    pub modo_repeticion: ModoRepeticion,
    pub autoplay: bool,
    pub canal_texto: Option<String>,
    pub canal_voz: Option<String>,
    pub posicion_segundos: u64,
    pub filtros: Vec<String>,
}

impl Cola {
    pub fn nueva(guild_id: &str) -> Self {
        Self {
            guild_id: guild_id.to_string(),
            canciones: Vec::new(),
            canciones_previas: Vec::new(),
            pausada: false,
            detenida: false,
            volumen: crate::sesiones::DEFAULT_VOLUME,
            modo_repeticion: ModoRepeticion::Apagado,
            autoplay: false,
            canal_texto: None,
            canal_voz: None,
            posicion_segundos: 0,
            filtros: Vec::new(),
        }
    }

    pub fn cancion_actual(&self) -> Option<&ElementoCola> {
        self.canciones.first()
    }

    pub fn cancion_actual_mut(&mut self) -> Option<&mut ElementoCola> {
        self.canciones.first_mut()
    }

    pub fn esta_activa(&self) -> bool {
        !self.detenida && !self.canciones.is_empty()
    }

    pub fn capacidad_restante(&self, max_queue_size: usize) -> usize {
        max_queue_size.saturating_sub(self.canciones.len())
    }
}

#[derive(Debug, Clone)]
pub struct EntradaDeshacer {
    pub canciones: Vec<ElementoCola>,
    pub expira_en: u64,
}

#[derive(Debug, Clone, Default)]
pub struct GestorColas {
    colas: Arc<Mutex<HashMap<String, Arc<Mutex<Cola>>>>>,
    deshacer: Arc<Mutex<HashMap<String, EntradaDeshacer>>>,
}

impl GestorColas {
    pub fn nuevo() -> Self {
        Self::default()
    }

    pub async fn obtener_cola(&self, guild_id: &str) -> Arc<Mutex<Cola>> {
        let mut map = self.colas.lock().await;
        map.entry(guild_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(Cola::nueva(guild_id))))
            .clone()
    }

    pub async fn eliminar_cola(&self, guild_id: &str) {
        let mut map = self.colas.lock().await;
        map.remove(guild_id);
    }

    pub async fn colas_activas(&self) -> Vec<Arc<Mutex<Cola>>> {
        let map = self.colas.lock().await;
        map.values().cloned().collect()
    }

    pub async fn tiene_cola(&self, guild_id: &str) -> bool {
        let map = self.colas.lock().await;
        map.contains_key(guild_id)
    }

    pub async fn guardar_deshacer(
        &self,
        guild_id: &str,
        canciones: Vec<ElementoCola>,
        expira_en: u64,
    ) {
        let mut map = self.deshacer.lock().await;
        map.insert(
            guild_id.to_string(),
            EntradaDeshacer {
                canciones,
                expira_en,
            },
        );
    }

    pub async fn obtener_deshacer(&self, guild_id: &str) -> Option<EntradaDeshacer> {
        let mut map = self.deshacer.lock().await;
        map.remove(guild_id)
    }

    pub async fn limpiar_deshacer_expirados(&self, ahora_ms: u64) {
        let mut map = self.deshacer.lock().await;
        map.retain(|_, v| ahora_ms <= v.expira_en);
    }
}
