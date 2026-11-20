use crate::alucinacion::SegmentoWhisper;
use std::time::{Duration, Instant};

pub const GROQ_API_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
pub const GROQ_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
pub const DEFAULT_GROQ_MODEL: &str = "whisper-large-v3-turbo";
pub const DEFAULT_TIMEOUT_MS: u64 = 10_000;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub struct Transcripcion {
    pub texto: String,
    pub ms: u64,
    pub segmentos: Vec<SegmentoWhisper>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroqError {
    SinClave,
    Estado(u16, String),
    Tiempo(u64),
    Red(String),
}

impl std::fmt::Display for GroqError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SinClave => write!(f, "Groq API Key is not configured"),
            Self::Estado(status, body) => {
                write!(f, "Groq STT failed with status {}: {}", status, body)
            }
            Self::Tiempo(ms) => write!(f, "Groq STT timed out after {}ms", ms),
            Self::Red(msg) => write!(f, "Groq network error: {}", msg),
        }
    }
}

impl std::error::Error for GroqError {}

#[derive(Debug, Clone)]
pub struct GroqConfig {
    pub api_url: String,
    pub api_key: String,
    pub model: String,
    pub language: Option<String>,
    pub prompt: Option<String>,
    pub timeout_ms: u64,
}

impl Default for GroqConfig {
    fn default() -> Self {
        Self {
            api_url: GROQ_API_URL.to_string(),
            api_key: String::new(),
            model: DEFAULT_GROQ_MODEL.to_string(),
            language: Some("es".to_string()),
            prompt: None,
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }
}

#[derive(serde::Deserialize)]
struct GroqVerboseResponse {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub segments: Vec<SegmentoWhisper>,
}

pub async fn transcribir_audio(
    wav_buffer: &[u8],
    config: &GroqConfig,
) -> Result<Transcripcion, GroqError> {
    if config.api_key.trim().is_empty() {
        return Err(GroqError::SinClave);
    }
    if wav_buffer.len() <= 44 {
        return Ok(Transcripcion::default());
    }

    let file_part = reqwest::multipart::Part::bytes(wav_buffer.to_vec())
        .file_name("command.wav")
        .mime_str("audio/wav")
        .map_err(|e| GroqError::Red(e.to_string()))?;

    let mut form = reqwest::multipart::Form::new()
        .part("file", file_part)
        .text("model", config.model.clone())
        .text("response_format", "verbose_json")
        .text("temperature", "0.0");

    if let Some(lang) = &config.language {
        if !lang.is_empty() {
            form = form.text("language", lang.clone());
        }
    }
    if let Some(prompt) = &config.prompt {
        if !prompt.is_empty() {
            form = form.text("prompt", prompt.clone());
        }
    }

    tracing::info!(model = %config.model, "Sending Groq transcription request");
    let start_time = Instant::now();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(config.timeout_ms))
        .build()
        .map_err(|e| GroqError::Red(e.to_string()))?;

    let request = client
        .post(&config.api_url)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", config.api_key),
        )
        .header(reqwest::header::USER_AGENT, GROQ_USER_AGENT)
        .multipart(form);

    let response = match request.send().await {
        Ok(res) => res,
        Err(err) => {
            if err.is_timeout() {
                tracing::error!(
                    timeout_ms = config.timeout_ms,
                    "Groq transcription request timed out"
                );
                return Err(GroqError::Tiempo(config.timeout_ms));
            }
            return Err(GroqError::Red(err.to_string()));
        }
    };

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        tracing::warn!(
            status = status.as_u16(),
            error_body = %error_body,
            "Groq transcription request failed"
        );
        return Err(GroqError::Estado(status.as_u16(), error_body));
    }

    let parsed: GroqVerboseResponse = response
        .json()
        .await
        .map_err(|e| GroqError::Red(e.to_string()))?;
    let elapsed_ms = start_time.elapsed().as_millis() as u64;
    let text = parsed.text.trim().to_string();

    tracing::debug!(text = %text, elapsed_ms, model = %config.model, "Groq transcription successful");

    Ok(Transcripcion {
        texto: text,
        ms: elapsed_ms,
        segmentos: parsed.segments,
    })
}
