use nucleo::Config;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn get_project_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .expect("Failed to locate project root")
        .to_path_buf()
}

#[test]
fn test_config_paridad_con_node() {
    let root = get_project_root();
    let casos_dir = root
        .join("rust")
        .join("crates")
        .join("nucleo")
        .join("tests")
        .join("data")
        .join("env-casos");

    let entries = fs::read_dir(&casos_dir).expect("Failed to read env-casos dir");
    let mut case_files = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().map(|e| e == "env").unwrap_or(false) {
            case_files.push(p);
        }
    }
    assert!(!case_files.is_empty(), "No .env case files found");

    for env_file in case_files {
        let json_file = env_file.with_extension("json");
        assert!(
            json_file.exists(),
            "Missing JSON reference file for {:?}",
            env_file
        );
        let json_str = fs::read_to_string(&json_file).expect("Failed to read json ref");
        let ref_val: Value = serde_json::from_str(&json_str).expect("Invalid json ref");

        let mut map = std::collections::HashMap::new();
        if let Ok(content) = fs::read_to_string(&env_file) {
            map.extend(nucleo::env::parse_dotenv(&content));
        }

        let config = Config::from_map(root.clone(), &map);

        assert_eq!(config.debug, ref_val["debug"].as_bool().unwrap());
        assert_eq!(config.no_color, ref_val["noColor"].as_bool().unwrap());
        assert_eq!(config.shard_id.as_deref(), ref_val["shardId"].as_str());
        let ref_dev_users: Vec<String> = ref_val["devUserIds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(config.dev_user_ids, ref_dev_users);

        assert_eq!(
            config.bot.status,
            ref_val["bot"]["status"].as_str().unwrap()
        );
        assert_eq!(
            config.bot.color,
            ref_val["bot"]["color"].as_u64().unwrap() as u32
        );
        assert_eq!(
            config.bot.default_volume,
            ref_val["bot"]["defaultVolume"].as_u64().unwrap() as u32
        );
        assert_eq!(
            config.bot.max_queue_size,
            ref_val["bot"]["maxQueueSize"].as_u64().unwrap() as u32
        );
        assert_eq!(
            config.bot.max_playlist_size,
            ref_val["bot"]["maxPlaylistSize"].as_u64().unwrap() as u32
        );
        assert_eq!(
            config.bot.leave_on_empty_ms,
            ref_val["bot"]["leaveOnEmptyMs"].as_u64().unwrap()
        );
        assert_eq!(
            config.bot.leave_on_finish_ms,
            ref_val["bot"]["leaveOnFinishMs"].as_u64().unwrap()
        );
        assert_eq!(
            config.bot.panel_refresh_ms,
            ref_val["bot"]["panelRefreshMs"].as_u64().unwrap()
        );
        assert_eq!(
            config.bot.default_language,
            ref_val["bot"]["defaultLanguage"].as_str().unwrap()
        );

        assert_eq!(
            config.ytdlp.concurrency,
            ref_val["ytdlp"]["concurrency"].as_u64().unwrap() as u32
        );
        assert_eq!(
            config.ytdlp.timeout_ms,
            ref_val["ytdlp"]["timeoutMs"].as_u64().unwrap()
        );

        assert_eq!(
            config.innertube.hl,
            ref_val["innertube"]["hl"].as_str().unwrap()
        );
        assert_eq!(
            config.innertube.gl,
            ref_val["innertube"]["gl"].as_str().unwrap()
        );

        assert_eq!(
            config.api.enabled,
            ref_val["api"]["enabled"].as_bool().unwrap()
        );
        assert_eq!(
            config.api.port,
            ref_val["api"]["port"].as_u64().unwrap() as u16
        );
        assert_eq!(
            config.api.rate_limit_per_minute,
            ref_val["api"]["rateLimitPerMinute"].as_u64().unwrap() as u32
        );
        assert_eq!(
            config.api.auto_tunnel,
            ref_val["api"]["autoTunnel"].as_bool().unwrap()
        );

        let ref_origins: Vec<String> = ref_val["api"]["allowedOrigins"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(config.api.allowed_origins, ref_origins);

        let ref_proxies: Vec<String> = ref_val["proxies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(config.proxies, ref_proxies);

        assert_eq!(
            config.voice.enabled,
            ref_val["voice"]["enabled"].as_bool().unwrap()
        );
        assert_eq!(
            config.voice.default_wake_word,
            ref_val["voice"]["defaultWakeWord"].as_str().unwrap()
        );
        assert_eq!(
            config.voice.silence_duration_ms,
            ref_val["voice"]["silenceDurationMs"].as_u64().unwrap()
        );
        assert_eq!(
            config.voice.model,
            ref_val["voice"]["model"].as_str().unwrap()
        );
        assert_eq!(
            config.voice.prompt,
            ref_val["voice"]["prompt"].as_str().unwrap()
        );
        assert_eq!(
            config.voice.offline_gating,
            ref_val["voice"]["offlineGating"].as_bool().unwrap()
        );
        assert_eq!(
            config.voice.min_rms,
            ref_val["voice"]["minRms"].as_u64().unwrap() as u32
        );
        assert_eq!(
            config.voice.groq_cooldown_ms,
            ref_val["voice"]["groqCooldownMs"].as_u64().unwrap()
        );
        assert_eq!(
            config.voice.tts_feedback_enabled,
            ref_val["voice"]["ttsFeedbackEnabled"].as_bool().unwrap()
        );
        assert_eq!(
            config.voice.allow_bot_input,
            ref_val["voice"]["allowBotInput"].as_bool().unwrap()
        );
        assert_eq!(
            config.voice.sound_bot_id,
            ref_val["voice"]["soundBotId"].as_str().unwrap()
        );

        assert_eq!(
            config.sharding.total_shards,
            ref_val["sharding"]["totalShards"].as_str().unwrap()
        );
        let config_shard_list_json = serde_json::to_value(&config.sharding.shard_list).unwrap();
        assert_eq!(
            config_shard_list_json, ref_val["sharding"]["shardList"],
            "shardList parity mismatch in {:?}",
            env_file
        );
        assert_eq!(
            config.sharding.mode,
            ref_val["sharding"]["mode"].as_str().unwrap()
        );
        assert_eq!(
            config.sharding.managed,
            ref_val["sharding"]["managed"].as_bool().unwrap()
        );
        assert_eq!(
            config.sharding.respawn,
            ref_val["sharding"]["respawn"].as_bool().unwrap()
        );
        assert_eq!(
            config.sharding.spawn_delay,
            ref_val["sharding"]["spawnDelay"].as_u64().unwrap()
        );
        assert_eq!(
            config.sharding.spawn_timeout,
            ref_val["sharding"]["spawnTimeout"].as_u64().unwrap()
        );
    }
}
