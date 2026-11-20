use crate::env::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub use crate::config_model::{
    ApiConfig, BotConfig, Config, DiscordConfig, InnertubeConfig, ShardList, ShardingConfig,
    SpotifyConfig, SupabaseConfig, VoiceConfig, YtdlpConfig,
};

pub fn parse_shard_list(value: &str) -> ShardList {
    if value == "auto" {
        return ShardList::Auto("auto".to_string());
    }
    let ids: Vec<u32> = value
        .split(',')
        .filter_map(|s| {
            let trimmed = s.trim();
            parse_js_int(trimmed).filter(|&n| n >= 0).map(|n| n as u32)
        })
        .collect();
    if ids.is_empty() {
        ShardList::Auto("auto".to_string())
    } else {
        ShardList::Ids(ids)
    }
}

impl Config {
    pub fn from_map(root: PathBuf, map: &HashMap<String, String>) -> Self {
        let resolve_dir = |rel: &str, fallback: &str| -> PathBuf {
            let path_str = get_env(map, rel, fallback);
            let p = Path::new(path_str);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                root.join(p)
            }
        };

        let dev_users = {
            let direct = get_env_list(map, "DEV_USER_IDS");
            if !direct.is_empty() {
                direct
            } else {
                let next_dev = get_env_list(map, "NEXT_PUBLIC_DEV_USER_IDS");
                if !next_dev.is_empty() {
                    next_dev
                } else {
                    vec!["472144161357824000".to_string()]
                }
            }
        };

        let shard_raw = get_env(map, "SHARDS", "");
        let shard_id = if shard_raw.is_empty() {
            None
        } else {
            Some(shard_raw.to_string())
        };

        let guild_raw = get_env(map, "GUILD_ID", "");
        let guild_id = if guild_raw.is_empty() {
            None
        } else {
            Some(guild_raw.to_string())
        };

        let total_shards_raw = get_env(map, "TOTAL_SHARDS", "auto");
        let total_shards = if total_shards_raw == "auto" {
            "auto".to_string()
        } else if let Some(n) = parse_js_int(total_shards_raw) {
            if n > 0 {
                n.to_string()
            } else {
                "auto".to_string()
            }
        } else {
            "auto".to_string()
        };

        let supabase_key_direct = get_env(map, "SUPABASE_KEY", "");
        let supabase_key = if !supabase_key_direct.is_empty() {
            supabase_key_direct.to_string()
        } else {
            get_env(map, "SUPABASE_ANON_KEY", "").to_string()
        };

        Self {
            data_dir: resolve_dir("DATA_DIR", "data"),
            bin_dir: resolve_dir("BIN_DIR", "bin"),
            debug: get_env_bool(map, "DEBUG", false),
            no_color: !get_env(map, "NO_COLOR", "").is_empty(),
            shard_id,
            dev_user_ids: dev_users,
            discord: DiscordConfig {
                token: get_env(map, "DISCORD_TOKEN", "").to_string(),
                client_id: get_env(map, "CLIENT_ID", "").to_string(),
                guild_id,
            },
            bot: BotConfig {
                status: get_env(map, "STATUS", "/play").to_string(),
                color: parse_color(get_env(map, "EMBED_COLOR", "#5865F2")),
                support_server: get_env(map, "SUPPORT_SERVER", "").to_string(),
                website: get_env(map, "WEBSITE", "").to_string(),
                default_language: get_env(map, "DEFAULT_LANGUAGE", "en").to_string(),
                default_volume: get_env_int(map, "DEFAULT_VOLUME", 100, Some(1), Some(100)) as u32,
                max_volume: 100,
                max_queue_size: get_env_int(map, "MAX_QUEUE_SIZE", 500, Some(1), None) as u32,
                max_playlist_size: get_env_int(map, "MAX_PLAYLIST_SIZE", 200, Some(1), None) as u32,
                leave_on_empty_ms: (get_env_int(map, "LEAVE_ON_EMPTY", 60, Some(0), None) * 1000) as u64,
                leave_on_finish_ms: (get_env_int(map, "LEAVE_ON_FINISH", 120, Some(0), None) * 1000) as u64,
                panel_refresh_ms: (get_env_int(map, "PANEL_REFRESH", 20, Some(0), None) * 1000) as u64,
            },
            spotify: SpotifyConfig {
                client_id: get_env(map, "SPOTIFY_CLIENT_ID", "").to_string(),
                client_secret: get_env(map, "SPOTIFY_CLIENT_SECRET", "").to_string(),
            },
            supabase: SupabaseConfig {
                url: get_env(map, "SUPABASE_URL", "").to_string(),
                key: supabase_key,
                service_role_key: get_env(map, "SUPABASE_SERVICE_ROLE_KEY", "").to_string(),
            },
            ytdlp: YtdlpConfig {
                path: get_env(map, "YTDLP_PATH", "").to_string(),
                cookies_file: get_env(map, "COOKIES_FILE", "").to_string(),
                cookies_from_browser: get_env(map, "COOKIES_FROM_BROWSER", "").to_string(),
                po_token: get_env(map, "YOUTUBE_PO_TOKEN", "").to_string(),
                pot_generator: get_env(map, "POT_GENERATOR", "").to_string(),
                pot_generator_path: get_env(map, "POT_GENERATOR_PATH", "").to_string(),
                extractor_args: get_env(map, "YTDLP_EXTRACTOR_ARGS", "").to_string(),
                concurrency: get_env_int(map, "YTDLP_CONCURRENCY", 4, Some(1), Some(32)) as u32,
                timeout_ms: (get_env_int(map, "YTDLP_TIMEOUT", 30, Some(5), None) * 1000) as u64,
                download_base: get_env(map, "YTDLP_DOWNLOAD_BASE", "").to_string(),
            },
            innertube: InnertubeConfig {
                hl: get_env(map, "INNERTUBE_HL", "es").to_string(),
                gl: get_env(map, "INNERTUBE_GL", "CO").to_string(),
            },
            ffmpeg_path: get_env(map, "FFMPEG_PATH", "").to_string(),
            proxies: get_env_list(map, "PROXY_URL"),
            sharding: ShardingConfig {
                total_shards,
                shard_list: parse_shard_list(get_env(map, "SHARD_LIST", "auto")),
                mode: if get_env(map, "SHARD_MODE", "process") == "worker" {
                    "worker".to_string()
                } else {
                    "process".to_string()
                },
                managed: get_env_bool(map, "SHARDING_MANAGER", false),
                respawn: get_env_bool(map, "SHARD_RESPAWN", true),
                spawn_delay: get_env_int(map, "SHARD_SPAWN_DELAY", 5500, Some(0), None) as u64,
                spawn_timeout: get_env_int(map, "SHARD_SPAWN_TIMEOUT", 60000, Some(5000), None) as u64,
            },
            api: ApiConfig {
                enabled: get_env_bool(map, "API_ENABLED", true),
                port: get_env_int(map, "API_PORT", 3001, Some(1024), Some(65535)) as u16,
                secret: get_env(map, "API_SECRET", "").to_string(),
                allowed_origins: get_env_list(map, "API_ALLOWED_ORIGINS"),
                rate_limit_per_minute: get_env_int(map, "API_RATE_LIMIT_PER_MINUTE", 120, Some(1), Some(100000)) as u32,
                public_url: get_env(map, "PUBLIC_API_URL", "").to_string(),
                auto_tunnel: get_env_bool(map, "AUTO_TUNNEL", false),
            },
            voice: VoiceConfig {
                enabled: get_env_bool(map, "VOICE_ENABLED", true),
                default_wake_word: get_env(map, "VOICE_DEFAULT_WAKE_WORD", "ignite").to_lowercase(),
                groq_api_key: get_env(map, "GROQ_API_KEY", "").to_string(),
                silence_duration_ms: get_env_int(map, "VOICE_SILENCE_DURATION_MS", 500, Some(400), Some(3000)) as u64,
                model: get_env(map, "GROQ_VOICE_MODEL", "whisper-large-v3-turbo").to_string(),
                prompt: get_env(
                    map,
                    "VOICE_PROMPT",
                    "Ignite, mierdabot, bot mierda, reproduce, pon, toca, pausa, salta, siguiente, anterior, agrega, añade, cola, lista, volumen, activa autoplay, desactiva autoplay, autoplay on, autoplay off.",
                )
                .to_string(),
                offline_gating: get_env_bool(map, "VOICE_OFFLINE_GATING", true),
                min_rms: get_env_int(map, "VOICE_MIN_RMS", 200, Some(0), Some(5000)) as u32,
                groq_cooldown_ms: get_env_int(map, "VOICE_GROQ_COOLDOWN_MS", 2000, Some(0), Some(30000)) as u64,
                sound_api_url: get_env(
                    map,
                    "IGNITE_SOUND_API_URL",
                    "https://wsnbbnbdc7.execute-api.us-east-2.amazonaws.com/sound",
                )
                .to_string(),
                sound_api_secret: get_env(map, "IGNITE_SOUND_API_SECRET", "").to_string(),
                tts_feedback_enabled: get_env_bool(map, "VOICE_TTS_FEEDBACK_ENABLED", true),
                allow_bot_input: get_env_bool(map, "VOICE_ALLOW_BOT_INPUT", false),
                sound_bot_id: get_env(map, "IGNITE_SOUND_BOT_ID", "1558180312050180317").to_string(),
            },
            root,
        }
    }

    pub fn client_id(&self) -> &str {
        &self.discord.client_id
    }

    pub fn cookies_file(&self) -> &str {
        &self.ytdlp.cookies_file
    }

    pub fn download_base(&self) -> String {
        std::env::var("YTDLP_DOWNLOAD_BASE").unwrap_or_else(|_| self.ytdlp.download_base.clone())
    }

    pub fn invite(&self) -> String {
        self.bot.invite(&self.discord.client_id)
    }
}
