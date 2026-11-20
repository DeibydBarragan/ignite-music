use crate::groq::{
    transcribir_audio, GroqConfig, Transcripcion, DEFAULT_GROQ_MODEL, DEFAULT_TIMEOUT_MS,
    GROQ_API_URL,
};
use crate::vosk_modelo::VoskModelManager;
use std::time::Instant;

pub const DEFAULT_PROMPT: &str = "Ignite, mierdabot, bot mierda, reproduce, pon, toca, pausa, salta, siguiente, anterior, agrega, añade, cola, lista, volumen, activa autoplay, desactiva autoplay, autoplay on, autoplay off.";

#[derive(Debug, Clone, Default)]
pub struct SttOptions {
    pub api_key: Option<String>,
    pub language: Option<String>,
    pub prompt: Option<String>,
    pub model: Option<String>,
    pub timeout_ms: Option<u64>,
    pub api_url: Option<String>,
    pub guild_id: Option<String>,
    pub user_id: Option<String>,
}

pub async fn transcribir_enunciado(
    wav_buffer: &[u8],
    pcm16k: &[u8],
    options: &SttOptions,
    vosk: Option<&VoskModelManager>,
) -> Transcripcion {
    if let Some(resultado) = transcribir_groq(wav_buffer, options).await {
        return resultado;
    }
    vosk.and_then(|modelo| transcribir_vosk(pcm16k, modelo))
        .unwrap_or_default()
}

pub async fn transcribir_groq(wav_buffer: &[u8], options: &SttOptions) -> Option<Transcripcion> {
    if let Some(key) = options.api_key.as_ref().filter(|k| !k.trim().is_empty()) {
        let prompt_to_use = options
            .prompt
            .clone()
            .unwrap_or_else(|| DEFAULT_PROMPT.to_string());
        let groq_cfg = GroqConfig {
            api_url: options
                .api_url
                .clone()
                .unwrap_or_else(|| GROQ_API_URL.to_string()),
            api_key: key.clone(),
            model: options
                .model
                .clone()
                .unwrap_or_else(|| DEFAULT_GROQ_MODEL.to_string()),
            language: options.language.clone().or_else(|| Some("es".to_string())),
            prompt: Some(prompt_to_use),
            timeout_ms: options.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS),
        };

        match transcribir_audio(wav_buffer, &groq_cfg).await {
            Ok(res) if !res.texto.is_empty() => {
                tracing::info!(
                    guild_id = ?options.guild_id,
                    user_id = ?options.user_id,
                    elapsedMs = res.ms,
                    "Voice input transcribed by Groq Whisper"
                );
                return Some(res);
            }
            Ok(_) => {}
            Err(err) => {
                tracing::warn!(
                    err = %err,
                    guild_id = ?options.guild_id,
                    user_id = ?options.user_id,
                    "Groq STT failed; falling back to local Vosk"
                );
            }
        }
    }
    None
}

pub fn transcribir_vosk(pcm16k: &[u8], v_mgr: &VoskModelManager) -> Option<Transcripcion> {
    let inicio = Instant::now();
    let texto = v_mgr.adquirir()?.procesar_enunciado(pcm16k);
    let ms = inicio.elapsed().as_millis() as u64;
    (!texto.is_empty()).then_some(Transcripcion {
        texto,
        ms,
        segmentos: Vec::new(),
    })
}
