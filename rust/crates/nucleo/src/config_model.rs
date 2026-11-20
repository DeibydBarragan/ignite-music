use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub token: String,
    pub client_id: String,
    pub guild_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    pub status: String,
    pub color: u32,
    pub support_server: String,
    pub website: String,
    pub default_language: String,
    pub default_volume: u32,
    pub max_volume: u32,
    pub max_queue_size: u32,
    pub max_playlist_size: u32,
    pub leave_on_empty_ms: u64,
    pub leave_on_finish_ms: u64,
    pub panel_refresh_ms: u64,
}

impl BotConfig {
    pub fn invite(&self, client_id: &str) -> String {
        if client_id.is_empty() {
            String::new()
        } else {
            format!(
                "https://discord.com/oauth2/authorize?client_id={}&permissions=3230720&scope=bot%20applications.commands",
                client_id
            )
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyConfig {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupabaseConfig {
    pub url: String,
    pub key: String,
    pub service_role_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YtdlpConfig {
    pub path: String,
    pub cookies_file: String,
    pub cookies_from_browser: String,
    pub po_token: String,
    pub pot_generator: String,
    pub pot_generator_path: String,
    pub extractor_args: String,
    pub concurrency: u32,
    pub timeout_ms: u64,
    pub download_base: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InnertubeConfig {
    pub hl: String,
    pub gl: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ShardList {
    Ids(Vec<u32>),
    Auto(String),
}

impl Default for ShardList {
    fn default() -> Self {
        Self::Auto("auto".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardingConfig {
    pub total_shards: String,
    pub shard_list: ShardList,
    pub mode: String,
    pub managed: bool,
    pub respawn: bool,
    pub spawn_delay: u64,
    pub spawn_timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub enabled: bool,
    pub port: u16,
    pub secret: String,
    pub allowed_origins: Vec<String>,
    pub rate_limit_per_minute: u32,
    pub public_url: String,
    pub auto_tunnel: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceConfig {
    pub enabled: bool,
    pub default_wake_word: String,
    pub groq_api_key: String,
    pub silence_duration_ms: u64,
    pub model: String,
    pub prompt: String,
    pub offline_gating: bool,
    pub min_rms: u32,
    pub groq_cooldown_ms: u64,
    pub sound_api_url: String,
    pub sound_api_secret: String,
    pub tts_feedback_enabled: bool,
    pub allow_bot_input: bool,
    pub sound_bot_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub root: PathBuf,
    pub data_dir: PathBuf,
    pub bin_dir: PathBuf,
    pub debug: bool,
    pub no_color: bool,
    pub shard_id: Option<String>,
    pub dev_user_ids: Vec<String>,
    pub discord: DiscordConfig,
    pub bot: BotConfig,
    pub spotify: SpotifyConfig,
    pub supabase: SupabaseConfig,
    pub ytdlp: YtdlpConfig,
    pub innertube: InnertubeConfig,
    pub ffmpeg_path: String,
    pub proxies: Vec<String>,
    pub sharding: ShardingConfig,
    pub api: ApiConfig,
    pub voice: VoiceConfig,
}
