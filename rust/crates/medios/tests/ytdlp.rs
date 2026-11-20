use medios::{
    base_args, clean_error, proxy_for, run_json, version, Limitador, Prioridad, RunJsonOptions,
    YtDlpSettings,
};
use std::collections::HashSet;
use std::fs;
use std::time::{Duration, Instant};

#[test]
fn test_base_args_configuration_and_precedence() {
    let settings_plain = YtDlpSettings {
        js_runtime: Some("node:C:\\node.exe".to_string()),
        ..Default::default()
    };
    let plain = base_args(&settings_plain, None, None);

    assert!(plain.contains(&"--force-ipv4".to_string()));
    assert!(plain.contains(&"--js-runtimes".to_string()));
    let js_idx = plain.iter().position(|a| a == "--js-runtimes").unwrap();
    assert_eq!(plain[js_idx + 1], "node:C:\\node.exe");
    assert!(!plain.contains(&"--proxy".to_string()));

    let timeout_idx = plain.iter().position(|a| a == "--socket-timeout").unwrap();
    assert_eq!(plain[timeout_idx + 1], "30");

    let retries_idx = plain.iter().position(|a| a == "--retries").unwrap();
    assert_eq!(plain[retries_idx + 1], "10");

    let frag_idx = plain
        .iter()
        .position(|a| a == "--fragment-retries")
        .unwrap();
    assert_eq!(plain[frag_idx + 1], "10");

    let sleep_idx = plain.iter().position(|a| a == "--retry-sleep").unwrap();
    assert_eq!(plain[sleep_idx + 1], "fragment:1");

    let buf_idx = plain.iter().position(|a| a == "--buffer-size").unwrap();
    assert_eq!(plain[buf_idx + 1], "16M");

    // Cookies file wins over browser cookies
    let settings_cookies = YtDlpSettings {
        cookies_file: Some("c.txt".to_string()),
        cookies_from_browser: Some("chrome".to_string()),
        ..Default::default()
    };
    let cookies = base_args(&settings_cookies, Some("http://p:1"), None);
    assert!(cookies.contains(&"--cookies".to_string()));
    assert!(!cookies.contains(&"--cookies-from-browser".to_string()));
    let proxy_idx = cookies.iter().position(|a| a == "--proxy").unwrap();
    assert_eq!(cookies[proxy_idx + 1], "http://p:1");

    // PO token & extractor args
    let settings_po = YtDlpSettings {
        po_token: Some("TOKEN".to_string()),
        extractor_args: Some("youtube:player_client=web_safari".to_string()),
        ..Default::default()
    };
    let po = base_args(&settings_po, None, None);
    let extractor_args: Vec<String> = po
        .iter()
        .enumerate()
        .filter(|(i, _)| *i > 0 && po[i - 1] == "--extractor-args")
        .map(|(_, val)| val.clone())
        .collect();
    assert_eq!(
        extractor_args,
        vec![
            "youtube:po_token=web.gvs+TOKEN;player_client=web,default".to_string(),
            "youtube:player_client=web_safari".to_string()
        ]
    );
}

#[test]
fn test_proxy_for_sticky_per_guild_and_distribution() {
    let proxies = vec![
        "http://a:1".to_string(),
        "http://b:1".to_string(),
        "http://c:1".to_string(),
    ];
    assert_eq!(proxy_for(Some("123"), &[]), None);
    assert_eq!(
        proxy_for(Some("123"), &["http://only:1".to_string()]),
        Some("http://only:1".to_string())
    );
    assert_eq!(
        proxy_for(Some("123"), &proxies),
        proxy_for(Some("123"), &proxies)
    );

    let mut used = HashSet::new();
    for i in 0..50u64 {
        let key = (100000000000000000u64 + i).to_string();
        if let Some(p) = proxy_for(Some(&key), &proxies) {
            used.insert(p);
        }
    }
    assert_eq!(used.len(), 3);
}

#[test]
fn test_clean_error_parses_last_error_and_prefixes() {
    let raw = "WARNING: x\nERROR: [youtube] abc123: Video unavailable";
    assert_eq!(clean_error(raw), "Video unavailable");

    let just_text = "just text";
    assert_eq!(clean_error(just_text), "just text");

    let empty = "";
    assert_eq!(clean_error(empty), "yt-dlp failed");

    let long_error = format!("ERROR: {}", "a".repeat(400));
    assert_eq!(clean_error(&long_error).len(), 300);
}

#[tokio::test]
async fn test_run_json_parses_stdout_even_with_warnings() {
    let fake_bin = env!("CARGO_BIN_EXE_fake-ytdlp");
    let temp_dir = tempfile::tempdir().unwrap();
    let log_path = temp_dir.path().join("ytdlp.log");
    std::env::set_var("FAKE_YTDLP_LOG", &log_path);
    std::env::set_var("FAKE_YTDLP_LOG_MATCH", "ok-target");

    let limiter = Limitador::new(2);
    let base = vec!["--ignore-config".to_string()];
    let extra = vec!["--flat-playlist".to_string()];

    let info = run_json(
        fake_bin,
        "ok-target",
        &limiter,
        RunJsonOptions {
            base_args: &base,
            extra_args: &extra,
            prioridad: Prioridad::Alta,
            timeout: Duration::from_secs(5),
            queue_timeout: Duration::from_secs(5),
        },
    )
    .await
    .expect("run_json ok");

    assert_eq!(info["target"], "ok-target");
    assert_eq!(info["id"], "abcdefghijk");

    let log_content = fs::read_to_string(&log_path).unwrap();
    std::env::remove_var("FAKE_YTDLP_LOG");
    std::env::remove_var("FAKE_YTDLP_LOG_MATCH");
    let matching_line = log_content
        .trim()
        .lines()
        .find(|l| l.contains("\"ok-target\""))
        .expect("log line for ok-target");
    let logged_args: Vec<String> = serde_json::from_str(matching_line).unwrap();

    let last_three = &logged_args[logged_args.len() - 3..];
    assert_eq!(last_three, &["--dump-single-json", "--", "ok-target"]);
    assert!(logged_args.contains(&"--ignore-config".to_string()));
}

#[tokio::test]
async fn test_run_json_rejects_with_clean_message_on_failure() {
    let fake_bin = env!("CARGO_BIN_EXE_fake-ytdlp");
    let limiter = Limitador::new(2);

    let err = run_json(
        fake_bin,
        "fail",
        &limiter,
        RunJsonOptions {
            timeout: Duration::from_secs(5),
            queue_timeout: Duration::from_secs(5),
            ..Default::default()
        },
    )
    .await
    .unwrap_err();

    assert!(err.mensaje.contains("Sign in to confirm you’re not a bot"));
    assert!(err.stderr.contains("WARNING"));

    let err_invalid = run_json(
        fake_bin,
        "invalid",
        &limiter,
        RunJsonOptions {
            timeout: Duration::from_secs(5),
            queue_timeout: Duration::from_secs(5),
            ..Default::default()
        },
    )
    .await
    .unwrap_err();

    assert_eq!(err_invalid.mensaje, "yt-dlp returned invalid JSON");
}

#[tokio::test]
async fn test_run_json_kills_process_on_timeout() {
    let fake_bin = env!("CARGO_BIN_EXE_fake-ytdlp");
    let limiter = Limitador::new(2);

    let started = Instant::now();
    let err = run_json(
        fake_bin,
        "slow",
        &limiter,
        RunJsonOptions {
            timeout: Duration::from_millis(200),
            queue_timeout: Duration::from_millis(200),
            ..Default::default()
        },
    )
    .await
    .unwrap_err();

    assert!(err.mensaje.contains("timed out"));
    assert!(started.elapsed() < Duration::from_millis(2500));
}

#[tokio::test]
async fn test_version_reports_binary_version() {
    let fake_bin = env!("CARGO_BIN_EXE_fake-ytdlp");
    let ver = version(fake_bin).await.expect("version");
    assert_eq!(ver, "2026.09.01");

    let err = version("non_existent_binary_xyz_123").await.unwrap_err();
    assert_eq!(err, "not found");
}
