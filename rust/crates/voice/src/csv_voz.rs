//! CSV persistence for voice event logs.
//!
//! Flushes batches of voice interaction logs to CSV files per guild
//! with formula injection protection and automatic retention cleanup.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crate::registro_voz::EntradaRegistroVoz;

pub const CSV_BATCH_SIZE: usize = 50;

pub const CSV_COLUMNS: [&str; 16] = [
    "id",
    "timestamp",
    "guildId",
    "userId",
    "audioBytes",
    "voskHeard",
    "wakeWordSpotted",
    "matchedWakeWord",
    "gateReason",
    "groqEngaged",
    "groqText",
    "groqElapsedMs",
    "commandId",
    "query",
    "ttsMessage",
    "status",
];

/// Escapes a raw string into standard CSV cell format.
pub fn to_csv_cell(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn starts_with_formula_char(s: &str) -> bool {
    s.starts_with(['=', '+', '-', '@', '\t', '\r'])
}

/// Formats an optional field, applying formula injection protection if free text.
pub fn format_cell(value: Option<&str>, is_free_text: bool) -> String {
    let Some(val) = value else {
        return String::new();
    };
    if val.is_empty() {
        return String::new();
    }
    if is_free_text && starts_with_formula_char(val) {
        let mut guarded = String::with_capacity(val.len() + 1);
        guarded.push('\'');
        guarded.push_str(val);
        to_csv_cell(&guarded)
    } else {
        to_csv_cell(val)
    }
}

/// Builds complete CSV content with header and row entries.
pub fn build_csv(entries: &[EntradaRegistroVoz]) -> String {
    let header = CSV_COLUMNS.join(",");
    if entries.is_empty() {
        return format!("{header}\n\n");
    }
    let mut out = String::new();
    out.push_str(&header);
    out.push('\n');
    for e in entries {
        let audio_bytes = e.audio_bytes.to_string();
        let wake_spotted = e.wake_word_spotted.to_string();
        let groq_engaged = e.groq_engaged.to_string();
        let groq_elapsed = e.groq_elapsed_ms.map(|ms| ms.to_string());

        let row = [
            format_cell(Some(&e.id), false),
            format_cell(Some(&e.timestamp), false),
            format_cell(Some(&e.guild_id), false),
            format_cell(e.user_id.as_deref(), false),
            to_csv_cell(&audio_bytes),
            format_cell(Some(&e.vosk_heard), true),
            to_csv_cell(&wake_spotted),
            format_cell(e.matched_wake_word.as_deref(), true),
            format_cell(e.gate_reason.as_deref(), false),
            to_csv_cell(&groq_engaged),
            format_cell(e.groq_text.as_deref(), true),
            format_cell(groq_elapsed.as_deref(), false),
            format_cell(e.command_id.as_deref(), false),
            format_cell(e.query.as_deref(), true),
            format_cell(e.tts_message.as_deref(), true),
            format_cell(Some(&e.status), false),
        ];
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}

/// Exporter that aggregates voice logs and writes CSV batches to disk.
pub struct ExportadorCsvVoz {
    dir: PathBuf,
    batch_size: usize,
    pendientes: Mutex<HashMap<String, Vec<EntradaRegistroVoz>>>,
}

pub type VoiceCsvExporter = ExportadorCsvVoz;

impl ExportadorCsvVoz {
    pub fn new(dir: impl Into<PathBuf>, batch_size: usize) -> Self {
        Self {
            dir: dir.into(),
            batch_size,
            pendientes: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_dir(dir: impl Into<PathBuf>) -> Self {
        Self::new(dir, CSV_BATCH_SIZE)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    pub fn add(&self, entry: &EntradaRegistroVoz) -> Option<PathBuf> {
        let mut pendientes = self.pendientes.lock().unwrap();
        let batch = pendientes.entry(entry.guild_id.clone()).or_default();
        batch.push(entry.clone());

        if batch.len() < self.batch_size {
            return None;
        }

        let to_flush: Vec<EntradaRegistroVoz> = batch.drain(..self.batch_size).collect();
        drop(pendientes);
        self.flush(&entry.guild_id, &to_flush)
    }

    pub fn flush(&self, guild_id: &str, entries: &[EntradaRegistroVoz]) -> Option<PathBuf> {
        if entries.is_empty() {
            return None;
        }
        if let Err(err) = std::fs::create_dir_all(&self.dir) {
            tracing::warn!(guild_id, %err, "No se pudo crear carpeta de logs de voz");
            return None;
        }

        let stamp = chrono::Utc::now()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            .replace([':', '.'], "-");
        let mut file_path = self.dir.join(format!("{guild_id}-{stamp}.csv"));
        let mut counter = 1;
        while file_path.exists() {
            file_path = self.dir.join(format!("{guild_id}-{stamp}_{counter}.csv"));
            counter += 1;
        }

        let content = build_csv(entries);
        match std::fs::write(&file_path, content) {
            Ok(()) => {
                tracing::info!(guild_id, count = entries.len(), file = %file_path.display(), "Lote de logs de voz volcado a CSV");
                Some(file_path)
            }
            Err(err) => {
                tracing::warn!(guild_id, %err, "No se pudo escribir el CSV de logs de voz");
                None
            }
        }
    }

    pub fn flush_all(&self) -> Vec<PathBuf> {
        let mut pendientes = self.pendientes.lock().unwrap();
        let to_flush: Vec<(String, Vec<EntradaRegistroVoz>)> = pendientes
            .iter_mut()
            .filter(|(_, batch)| !batch.is_empty())
            .map(|(guild_id, batch)| (guild_id.clone(), std::mem::take(batch)))
            .collect();
        drop(pendientes);

        let mut written = Vec::new();
        for (guild_id, batch) in to_flush {
            if let Some(path) = self.flush(&guild_id, &batch) {
                written.push(path);
            }
        }
        written
    }

    pub fn borrar_antiguos(&self, max_dias: u64) -> usize {
        self.borrar_mas_antiguos_que(Duration::from_secs(max_dias * 86_400))
    }

    pub fn iniciar_tarea_retencion(
        self: Arc<Self>,
        max_dias: u64,
        intervalo: Duration,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            self.borrar_antiguos(max_dias);
            let mut ticker = tokio::time::interval(intervalo);
            ticker.tick().await;
            loop {
                ticker.tick().await;
                self.borrar_antiguos(max_dias);
            }
        })
    }

    pub fn borrar_mas_antiguos_que(&self, duracion: Duration) -> usize {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return 0;
        };
        let now = SystemTime::now();
        let mut eliminados = 0;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("csv") {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let Ok(modified) = meta.modified() else {
                continue;
            };
            if let Ok(elapsed) = now.duration_since(modified) {
                if elapsed > duracion && std::fs::remove_file(&path).is_ok() {
                    eliminados += 1;
                }
            }
        }
        eliminados
    }
}
