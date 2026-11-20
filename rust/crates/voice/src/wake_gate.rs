use crate::admission::{GateEvaluation, WakeGateEvaluator};
use crate::vosk::{VoskError, VoskWrapper};
mod modelo;
use crate::wake_gate_gramatica::{build_grammar, has_voiced_tail, TAIL_SKIP_SEC};
use modelo::ModeloGramatical;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Veredicto {
    pub abierta: bool,
    pub motivo: String,
    pub oido: String,
    pub palabra: Option<String>,
    pub inicio_ms: Option<u32>,
    pub fin_ms: Option<u32>,
    pub no_soportadas: Option<Vec<String>>,
}

impl Veredicto {
    fn closed(
        motivo: &str,
        oido: String,
        palabra: Option<String>,
        no_soportadas: Option<Vec<String>>,
    ) -> Self {
        Self {
            abierta: false,
            motivo: motivo.into(),
            oido,
            palabra,
            inicio_ms: None,
            fin_ms: None,
            no_soportadas,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct VoskWord {
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub end: f64,
    #[serde(default)]
    pub word: String,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct VoskFinal {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub result: Vec<VoskWord>,
}

#[derive(Default)]
pub struct WakeGate {
    models: HashMap<String, ModeloGramatical>,
}

impl WakeGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn initialize(
        dll_path: &Path,
        english_model_dir: &Path,
        spanish_model_dir: Option<&Path>,
    ) -> Result<Self, VoskError> {
        let wrapper = Arc::new(VoskWrapper::init(dll_path)?);
        let mut models = HashMap::new();

        let en_model = ModeloGramatical::cargar(wrapper.clone(), english_model_dir)?;
        models.insert("en".to_string(), en_model);

        if let Some(es_dir) = spanish_model_dir {
            if es_dir.join("am").join("final.mdl").exists() {
                if let Ok(es_model) = ModeloGramatical::cargar(wrapper.clone(), es_dir) {
                    models.insert("es".to_string(), es_model);
                }
            }
        }

        Ok(Self { models })
    }

    pub fn is_ready(&self) -> bool {
        !self.models.is_empty()
    }

    fn has_word(&self, model_id: &str, word: &str) -> bool {
        self.models
            .get(model_id)
            .is_some_and(|modelo| modelo.has_word(word))
    }

    pub fn evaluar(&self, pcm16k: &[u8], palabras: &str) -> Veredicto {
        if !self.is_ready() {
            return Veredicto::closed("gate_unavailable", String::new(), None, None);
        }

        let pcm_i16: Vec<i16> = pcm16k
            .as_chunks::<2>()
            .0
            .iter()
            .copied()
            .map(i16::from_le_bytes)
            .collect();

        let plan_res = build_grammar(
            palabras,
            |m, w| self.has_word(m, w),
            |m| self.models.contains_key(m),
        );

        if plan_res.plans.is_empty() {
            return Veredicto::closed(
                "no_supported_wake_word",
                String::new(),
                None,
                Some(plan_res.unsupported),
            );
        }

        let mut heard_parts = Vec::new();
        let mut spotted = None;

        for plan in &plan_res.plans {
            let grammar_json = serde_json::to_string(&plan.grammar).unwrap_or_else(|_| "[]".into());
            let Some(final_raw) = self.models[&plan.model_id].reconocer(pcm16k, &grammar_json)
            else {
                continue;
            };

            let parsed: VoskFinal = serde_json::from_str(&final_raw).unwrap_or_default();
            let trimmed = parsed.text.trim();
            if !trimmed.is_empty() {
                heard_parts.push(trimmed.to_string());
            }

            for phrase in &plan.phrases {
                let n = phrase.tokens.len();
                if parsed.result.len() < n {
                    continue;
                }
                for i in 0..=(parsed.result.len() - n) {
                    let matches = phrase
                        .tokens
                        .iter()
                        .enumerate()
                        .all(|(k, tok)| parsed.result[i + k].word == *tok);
                    if !matches {
                        continue;
                    }
                    let last = &parsed.result[i + n - 1];
                    if spotted.is_none() {
                        spotted = Some(phrase.wake_word.clone());
                    }
                    let followed = (i + n < parsed.result.len())
                        || has_voiced_tail(&pcm_i16, last.end + TAIL_SKIP_SEC);
                    if followed {
                        return Veredicto {
                            abierta: true,
                            motivo: "ok".into(),
                            oido: heard_parts.join(" | "),
                            palabra: Some(phrase.wake_word.clone()),
                            inicio_ms: Some((parsed.result[i].start * 1000.0).round() as u32),
                            fin_ms: Some((last.end * 1000.0).round() as u32),
                            no_soportadas: None,
                        };
                    }
                }
            }
        }

        let heard = heard_parts.join(" | ");
        if let Some(wake_word) = spotted {
            Veredicto::closed("no_command_after", heard, Some(wake_word), None)
        } else {
            Veredicto::closed("no_wake_word", heard, None, None)
        }
    }

    pub async fn evaluar_async(self: Arc<Self>, pcm16k: Vec<u8>, palabras: String) -> Veredicto {
        tokio::task::spawn_blocking(move || self.evaluar(&pcm16k, &palabras))
            .await
            .unwrap_or_else(|_| Veredicto::closed("gate_unavailable", String::new(), None, None))
    }

    pub fn destroy(&mut self) {
        self.models.clear();
    }
}

impl WakeGateEvaluator for WakeGate {
    fn evaluate(&self, pcm16k: &[i16], wake_words: &str) -> GateEvaluation {
        let bytes: Vec<u8> = pcm16k.iter().flat_map(|s| s.to_le_bytes()).collect();
        let v = self.evaluar(&bytes, wake_words);
        GateEvaluation {
            passed: v.abierta,
            reason: Some(v.motivo),
            heard: Some(v.oido),
            wake_word: v.palabra,
            start_ms: v.inicio_ms.unwrap_or(0),
        }
    }
}
