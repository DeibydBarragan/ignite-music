//! In-memory voice event log store and SSE dispatcher.
//!
//! Maintains a per-guild ring buffer of recent voice interactions
//! and handles SSE broadcasting and CSV export dispatch.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::csv_voz::ExportadorCsvVoz;

pub const MAX_LOGS_PER_GUILD: usize = 100;

/// Normalized voice event entry stored in memory and exported to CSV/SSE.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntradaRegistroVoz {
    pub id: String,
    pub timestamp: String,
    pub guild_id: String,
    pub user_id: Option<String>,
    pub audio_bytes: u64,
    pub vosk_heard: String,
    pub wake_word_spotted: bool,
    pub matched_wake_word: Option<String>,
    pub gate_reason: Option<String>,
    pub groq_engaged: bool,
    pub groq_text: Option<String>,
    pub groq_elapsed_ms: Option<u64>,
    pub command_id: Option<String>,
    pub query: Option<String>,
    pub tts_message: Option<String>,
    pub status: String,
}

/// Unnormalized voice event payload received before assignment of ID and timestamp.
#[derive(Debug, Clone, Default)]
pub struct NuevaEntradaRegistroVoz {
    pub guild_id: String,
    pub user_id: Option<String>,
    pub audio_bytes: Option<u64>,
    pub vosk_heard: Option<String>,
    pub wake_word_spotted: Option<bool>,
    pub matched_wake_word: Option<String>,
    pub gate_reason: Option<String>,
    pub groq_engaged: Option<bool>,
    pub groq_text: Option<String>,
    pub groq_elapsed_ms: Option<u64>,
    pub command_id: Option<String>,
    pub query: Option<String>,
    pub tts_message: Option<String>,
    pub status: Option<String>,
}

impl NuevaEntradaRegistroVoz {
    pub fn new(guild_id: impl Into<String>) -> Self {
        Self {
            guild_id: guild_id.into(),
            ..Default::default()
        }
    }

    pub fn normalizar(self) -> EntradaRegistroVoz {
        EntradaRegistroVoz {
            id: generar_id(),
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            guild_id: self.guild_id,
            user_id: self.user_id,
            audio_bytes: self.audio_bytes.unwrap_or(0),
            vosk_heard: self.vosk_heard.unwrap_or_default(),
            wake_word_spotted: self.wake_word_spotted.unwrap_or(false),
            matched_wake_word: self.matched_wake_word,
            gate_reason: self.gate_reason,
            groq_engaged: self.groq_engaged.unwrap_or(false),
            groq_text: self.groq_text,
            groq_elapsed_ms: self.groq_elapsed_ms.filter(|&ms| ms > 0),
            command_id: self.command_id,
            query: self.query,
            tts_message: self.tts_message,
            status: self.status.unwrap_or_else(|| "filtered_out".to_string()),
        }
    }
}

fn generar_id() -> String {
    const CHARSET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::thread_rng();
    let suffix: String = (0..5)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("vlog_{now_ms}_{suffix}")
}

/// Trait for SSE event publishing.
pub trait PublicadorRegistroVoz: Send + Sync {
    fn publicar(&self, guild_id: &str, evento: &str, entrada: &EntradaRegistroVoz);
}

pub type VoiceLogPublisher = dyn PublicadorRegistroVoz;

/// Per-guild voice event log store backed by ring buffers.
pub struct AlmacenRegistroVoz {
    max_per_guild: usize,
    csv_exporter: Option<Arc<ExportadorCsvVoz>>,
    publicador: Option<Arc<dyn PublicadorRegistroVoz>>,
    logs: RwLock<HashMap<String, VecDeque<EntradaRegistroVoz>>>,
    csv_activos: RwLock<HashSet<String>>,
}

pub type VoiceEventLogStore = AlmacenRegistroVoz;

impl AlmacenRegistroVoz {
    pub fn new(
        max_per_guild: usize,
        csv_exporter: Option<Arc<ExportadorCsvVoz>>,
        publicador: Option<Arc<dyn PublicadorRegistroVoz>>,
    ) -> Self {
        Self {
            max_per_guild,
            csv_exporter,
            publicador,
            logs: RwLock::new(HashMap::new()),
            csv_activos: RwLock::new(HashSet::new()),
        }
    }

    pub fn with_exporter(csv_exporter: Arc<ExportadorCsvVoz>) -> Self {
        Self::new(MAX_LOGS_PER_GUILD, Some(csv_exporter), None)
    }

    pub fn fijar_csv(&self, servidor: impl Into<String>, activo: bool) {
        let mut guard = self.csv_activos.write().unwrap();
        let s = servidor.into();
        if activo {
            guard.insert(s);
        } else {
            guard.remove(&s);
        }
    }

    pub fn csv_activo(&self, servidor: &str) -> bool {
        self.csv_activos.read().unwrap().contains(servidor)
    }

    pub fn registrar(&self, entrada: NuevaEntradaRegistroVoz) -> EntradaRegistroVoz {
        let normalizada = entrada.normalizar();
        self.registrar_normalizada(normalizada)
    }

    pub fn registrar_normalizada(&self, log_entry: EntradaRegistroVoz) -> EntradaRegistroVoz {
        {
            let mut logs = self.logs.write().unwrap();
            let list = logs.entry(log_entry.guild_id.clone()).or_default();
            if list.len() >= self.max_per_guild {
                list.pop_front();
            }
            list.push_back(log_entry.clone());
        }

        if self.csv_activo(&log_entry.guild_id) {
            if let Some(ref exporter) = self.csv_exporter {
                let _ = exporter.add(&log_entry);
            }
        }

        if let Some(ref publ) = self.publicador {
            publ.publicar(&log_entry.guild_id, "voice.log", &log_entry);
        }

        log_entry
    }

    pub fn record(&self, entrada: NuevaEntradaRegistroVoz) -> EntradaRegistroVoz {
        self.registrar(entrada)
    }

    pub fn obtener(&self, servidor: &str, limite: Option<usize>) -> Vec<EntradaRegistroVoz> {
        let limit = limite.unwrap_or(50);
        let logs = self.logs.read().unwrap();
        let Some(list) = logs.get(servidor) else {
            return Vec::new();
        };
        list.iter().rev().take(limit).cloned().collect()
    }

    pub fn get_logs(&self, guild_id: &str, limit: Option<usize>) -> Vec<EntradaRegistroVoz> {
        self.obtener(guild_id, limit)
    }

    pub fn limpiar(&self, servidor: &str) {
        let mut logs = self.logs.write().unwrap();
        logs.remove(servidor);
    }

    pub fn clear(&self, guild_id: &str) {
        self.limpiar(guild_id);
    }

    pub fn flush_all(&self) -> Vec<PathBuf> {
        if let Some(ref exporter) = self.csv_exporter {
            exporter.flush_all()
        } else {
            Vec::new()
        }
    }

    pub fn iniciar_retencion(
        &self,
        max_dias: u64,
        intervalo: std::time::Duration,
    ) -> Option<tokio::task::JoinHandle<()>> {
        self.csv_exporter
            .as_ref()
            .map(|exp| exp.clone().iniciar_tarea_retencion(max_dias, intervalo))
    }
}

impl Default for AlmacenRegistroVoz {
    fn default() -> Self {
        Self::new(MAX_LOGS_PER_GUILD, None, None)
    }
}
