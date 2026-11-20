//! Admission control for captured voice utterances (Spec S-08).
//!
//! Decides if a speech segment may reach transcription (Groq / Whisper):
//!   1. Volume stage: drops near-silent audio (breathing, ambient hiss).
//!   2. Wake-word gate stage: confirms wake word presence (S-06).
//!   3. Cooldown stage: limits requests per user/guild.
//!
//! Admitted audio is trimmed via `slice_pcm` (S-07) preserving 250ms pre-roll.

use crate::audio::{pcm_rms, slice_pcm};
use std::collections::HashMap;
use std::sync::RwLock;
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct AdmissionConfig {
    pub min_rms: f64,
    pub offline_gating: bool,
    pub groq_cooldown_ms: u64,
}

impl Default for AdmissionConfig {
    fn default() -> Self {
        Self {
            min_rms: 200.0,
            offline_gating: true,
            groq_cooldown_ms: 2_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GateEvaluation {
    pub passed: bool,
    pub reason: Option<String>,
    pub heard: Option<String>,
    pub wake_word: Option<String>,
    pub start_ms: u32,
}

pub trait WakeGateEvaluator: Send + Sync {
    fn evaluate(&self, pcm16k: &[i16], wake_words: &str) -> GateEvaluation;
}

#[derive(Debug, Clone, PartialEq)]
pub enum AdmissionDecision<'a> {
    LowVolume {
        rms: f64,
    },
    GateFilteredOut {
        reason: Option<String>,
        heard: Option<String>,
    },
    RateLimited,
    Admitted {
        sliced_pcm: &'a [i16],
        gate_start_ms: u32,
        gate: Option<GateEvaluation>,
    },
}

pub struct AdmissionController {
    config: AdmissionConfig,
    last_groq_at: RwLock<HashMap<(u64, u64), u64>>,
}

impl AdmissionController {
    pub fn new(config: AdmissionConfig) -> Self {
        Self {
            config,
            last_groq_at: RwLock::new(HashMap::new()),
        }
    }

    /// Evaluates admission criteria for 16kHz mono PCM.
    pub fn admit<'a>(
        &self,
        guild_id: u64,
        user_id: u64,
        pcm16k: &'a [i16],
        wake_words: &str,
        evaluator: Option<&dyn WakeGateEvaluator>,
        now_ms: u64,
    ) -> AdmissionDecision<'a> {
        // Stage 0: Drop near-silent audio
        let rms = pcm_rms(pcm16k);
        if rms < self.config.min_rms {
            debug!(guild_id, user_id, rms, "Audio too quiet; discarded");
            return AdmissionDecision::LowVolume { rms };
        }

        // Stage 1: Wake-word gate
        let gate = if self.config.offline_gating {
            let Some(evaluator) = evaluator else {
                return AdmissionDecision::GateFilteredOut {
                    reason: Some("gate_unavailable".into()),
                    heard: None,
                };
            };
            let eval = evaluator.evaluate(pcm16k, wake_words);
            if !eval.passed {
                info!(
                    guild_id,
                    user_id,
                    reason = ?eval.reason,
                    heard = ?eval.heard,
                    "Wake-word gate closed; Groq not called"
                );
                return AdmissionDecision::GateFilteredOut {
                    reason: eval.reason,
                    heard: eval.heard,
                };
            }
            Some(eval)
        } else {
            None
        };
        let start_ms = gate.as_ref().map_or(0, |g| g.start_ms);

        // Stage 2: Rate limit / cooldown per user
        let mut ultimo = self.last_groq_at.write().unwrap();
        ultimo.retain(|_, t| now_ms.saturating_sub(*t) < self.config.groq_cooldown_ms);
        let std::collections::hash_map::Entry::Vacant(turno) = ultimo.entry((guild_id, user_id))
        else {
            return AdmissionDecision::RateLimited;
        };
        turno.insert(now_ms);
        drop(ultimo);

        // Stage 3: Audio slicing before wake word with 250ms pre-roll
        let sliced_pcm = if start_ms > 0 {
            slice_pcm(pcm16k, start_ms, 250)
        } else {
            pcm16k
        };

        AdmissionDecision::Admitted {
            sliced_pcm,
            gate_start_ms: start_ms,
            gate,
        }
    }
}

#[cfg(test)]
#[path = "admission_pruebas.rs"]
mod tests;
