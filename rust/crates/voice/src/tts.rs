use std::collections::HashMap;
use std::time::Duration;

pub const DEFAULT_SOUND_API_URL: &str =
    "https://wsnbbnbdc7.execute-api.us-east-2.amazonaws.com/sound";
pub const DEFAULT_TTS_VOICE: &str = "es-MX-DaliaNeural";
pub const DEFAULT_TTS_TIMEOUT_MS: u64 = 5000;

pub const DEFAULT_TTS_TEMPLATES: &[(&str, &str)] = &[
    ("play", "Reproduciendo {query}"),
    ("play_next", "Añadida {query} a continuación"),
    ("enqueue", "Añadida {query} a la lista"),
    ("skip", "Saltando canción"),
    ("pause", "Música pausada"),
    ("resume", "Reanudando música"),
    ("stop", "Reproducción detenida"),
    ("previous", "Canción anterior"),
    ("shuffle", "Lista en modo aleatorio"),
    ("autoplay_on", "Autoplay activado"),
    ("autoplay_off", "Autoplay desactivado"),
    ("volume", "Volumen ajustado al {query}"),
    ("queue", "Mostrando la lista"),
    ("hola_que_hace", "Hola que hace"),
];

pub fn interpolate_template(template: &str, params: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '{' {
            let mut key = String::new();
            let mut closed = false;
            for next_c in chars.by_ref() {
                if next_c == '}' {
                    closed = true;
                    break;
                }
                if next_c.is_alphanumeric() || next_c == '_' {
                    key.push(next_c);
                } else {
                    out.push('{');
                    out.push_str(&key);
                    out.push(next_c);
                    key.clear();
                    break;
                }
            }
            if closed {
                if let Some(val) = params.get(&key) {
                    out.push_str(val);
                }
            } else if !key.is_empty() {
                out.push('{');
                out.push_str(&key);
            }
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}

pub fn resolve_feedback_message(
    command_id: &str,
    params: &HashMap<String, String>,
    custom_templates: Option<&HashMap<String, String>>,
) -> Option<String> {
    if let Some(custom) = custom_templates {
        if let Some(tmpl) = custom.get(command_id) {
            return Some(interpolate_template(tmpl, params));
        }
    }
    for &(cmd, tmpl) in DEFAULT_TTS_TEMPLATES {
        if cmd == command_id {
            return Some(interpolate_template(tmpl, params));
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AvisoTts {
    pub guild_id: String,
    pub channel_id: String,
    pub user_id: Option<String>,
    pub command_id: String,
    pub params: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct OpcionesTts {
    pub enabled: Option<bool>,
    pub templates: Option<HashMap<String, String>>,
    pub api_url: Option<String>,
    pub api_secret: Option<String>,
    pub voice: Option<String>,
    pub rate: Option<f64>,
    pub pitch: Option<f64>,
    pub timeout_ms: Option<u64>,
}

#[derive(serde::Serialize)]
struct TtsRequestBody<'a> {
    text: &'a str,
    #[serde(rename = "guildId")]
    guild_id: &'a str,
    #[serde(rename = "channelId")]
    channel_id: &'a str,
    #[serde(rename = "userId", skip_serializing_if = "Option::is_none")]
    user_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    voice: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pitch: Option<f64>,
}

pub async fn send_tts_feedback(payload: &AvisoTts, options: Option<&OpcionesTts>) -> bool {
    let is_enabled = options.and_then(|o| o.enabled).unwrap_or(true);
    if !is_enabled {
        return false;
    }

    let default_url = DEFAULT_SOUND_API_URL.to_string();
    let api_url = options
        .and_then(|o| o.api_url.as_ref())
        .unwrap_or(&default_url);
    if api_url.is_empty() {
        tracing::debug!("TTS feedback skipped: soundApiUrl is not configured");
        return false;
    }

    if payload.guild_id.is_empty() || payload.channel_id.is_empty() {
        tracing::debug!("TTS feedback skipped: missing guildId or channelId");
        return false;
    }

    let custom = options.and_then(|o| o.templates.as_ref());
    let Some(message) = resolve_feedback_message(&payload.command_id, &payload.params, custom)
    else {
        return false;
    };

    let secret = options.and_then(|o| o.api_secret.as_deref());
    let endpoint = format!("{}/api/v1/tts/speak", api_url.trim_end_matches('/'));

    let body = TtsRequestBody {
        text: &message,
        guild_id: &payload.guild_id,
        channel_id: &payload.channel_id,
        user_id: payload.user_id.as_deref(),
        voice: options.and_then(|o| o.voice.as_deref()),
        rate: options.and_then(|o| o.rate),
        pitch: options.and_then(|o| o.pitch),
    };

    let timeout_ms = options
        .and_then(|o| o.timeout_ms)
        .unwrap_or(DEFAULT_TTS_TIMEOUT_MS);
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(err = %e, guild_id = %payload.guild_id, "Failed to build HTTP client for TTS");
            return false;
        }
    };

    let mut req = client.post(&endpoint).json(&body);
    if let Some(sec) = secret {
        req = req.header(reqwest::header::AUTHORIZATION, format!("Bearer {}", sec));
    }
    if let Some(uid) = &payload.user_id {
        req = req.header("X-User-Discord-Id", uid);
    }

    match req.send().await {
        Ok(res) if res.status().is_success() => {
            tracing::info!(
                guild_id = %payload.guild_id,
                command_id = %payload.command_id,
                message = %message,
                "TTS confirmation dispatched to ignite-sound"
            );
            true
        }
        Ok(res) => {
            let status = res.status().as_u16();
            let err_text = res.text().await.unwrap_or_default();
            tracing::warn!(
                status,
                err_text = %err_text,
                guild_id = %payload.guild_id,
                "ignite-sound TTS returned non-200"
            );
            false
        }
        Err(e) => {
            tracing::warn!(
                err = %e,
                guild_id = %payload.guild_id,
                "Failed to send TTS confirmation to ignite-sound"
            );
            false
        }
    }
}
