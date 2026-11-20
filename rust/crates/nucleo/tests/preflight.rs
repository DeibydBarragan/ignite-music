use nucleo::config::Config;
use nucleo::preflight::*;
use nucleo::report::Reporter;
use nucleo::token::{ApplicationBot, ApplicationInfo, TokenCheckResult, TokenChecker};
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::tempdir;

struct MockTokenChecker {
    result: TokenCheckResult,
    calls: Arc<AtomicUsize>,
}

impl MockTokenChecker {
    fn new(result: TokenCheckResult) -> (Self, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        (
            Self {
                result,
                calls: calls.clone(),
            },
            calls,
        )
    }
}

impl TokenChecker for MockTokenChecker {
    fn check<'a>(
        &'a self,
        _token: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = TokenCheckResult> + Send + 'a>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let res = match &self.result {
            TokenCheckResult::Ok(app) => TokenCheckResult::Ok(app.clone()),
            TokenCheckResult::Invalid => TokenCheckResult::Invalid,
            TokenCheckResult::Unreachable(e) => TokenCheckResult::Unreachable(e.clone()),
        };
        Box::pin(async move { res })
    }
}

fn valid_token_string() -> String {
    format!("{}.{}.{}", "A".repeat(26), "B".repeat(6), "C".repeat(38))
}

#[test]
fn test_validators() {
    assert!(is_snowflake("1011718919473610863"));
    assert!(!is_snowflake("abc"));
    assert!(!is_snowflake("123"));

    let token = valid_token_string();
    assert!(is_token_format(&token));
    assert!(!is_token_format("Bot invalid.token.shape"));
    assert!(!is_token_format(""));

    assert!(validate_proxy("http://user:pass@1.2.3.4:8080"));
    assert!(validate_proxy("socks5://host:1080"));
    assert!(!validate_proxy("ftp://host"));
    assert!(!validate_proxy("not a proxy"));
}

#[test]
fn test_ensure_env_file() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    assert!(!ensure_env_file(root, None));

    let example = root.join(".env.example");
    fs::write(&example, "DISCORD_TOKEN=\n").unwrap();

    assert!(ensure_env_file(root, None));
    let env_file = root.join(".env");
    assert_eq!(fs::read_to_string(&env_file).unwrap(), "DISCORD_TOKEN=\n");

    fs::write(&env_file, "DISCORD_TOKEN=mine\n").unwrap();
    assert!(!ensure_env_file(root, None));
    assert_eq!(
        fs::read_to_string(&env_file).unwrap(),
        "DISCORD_TOKEN=mine\n"
    );

    assert!(!ensure_env_file(root, Some("mine")));
    assert!(!ensure_env_file(root, Some("   ")));
}

#[tokio::test]
async fn test_preflight_token_valid_and_rejected() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let reporter = Reporter::new(true);

    let mut map = HashMap::new();
    map.insert("DISCORD_TOKEN".into(), valid_token_string());
    map.insert("API_SECRET".into(), "a".repeat(32));

    let mut config = Config::from_map(root.clone(), &map);
    let (checker, calls) = MockTokenChecker::new(TokenCheckResult::Ok(ApplicationInfo {
        id: "1011718919473610863".into(),
        name: "TestBot".into(),
        bot: Some(ApplicationBot {
            username: Some("TestBotUser".into()),
        }),
    }));

    let res = run_preflight(&mut config, &reporter, &checker).await;
    assert!(res.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(config.discord.client_id, "1011718919473610863");

    let (checker_invalid, _) = MockTokenChecker::new(TokenCheckResult::Invalid);
    let mut config_fail = Config::from_map(root.clone(), &map);
    let res_invalid = run_preflight(&mut config_fail, &reporter, &checker_invalid).await;
    assert!(res_invalid.is_err());
}

#[tokio::test]
async fn test_preflight_rule_9_total_shards() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let reporter = Reporter::new(true);

    let mut map = HashMap::new();
    map.insert("DISCORD_TOKEN".into(), valid_token_string());
    map.insert("API_SECRET".into(), "a".repeat(32));
    map.insert("TOTAL_SHARDS".into(), "2".into());

    let mut config = Config::from_map(root.clone(), &map);
    config.sharding.total_shards = "2".into();
    let (checker, _) = MockTokenChecker::new(TokenCheckResult::Ok(ApplicationInfo {
        id: "1011718919473610863".into(),
        name: "TestBot".into(),
        bot: None,
    }));

    let res = run_preflight(&mut config, &reporter, &checker).await;
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().problemas, 1);
}

#[tokio::test]
async fn test_preflight_api_secret_length() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let reporter = Reporter::new(true);

    let mut map = HashMap::new();
    map.insert("DISCORD_TOKEN".into(), valid_token_string());
    map.insert("API_SECRET".into(), "a".repeat(31));

    let mut config = Config::from_map(root.clone(), &map);
    let (checker, _) = MockTokenChecker::new(TokenCheckResult::Ok(ApplicationInfo {
        id: "1011718919473610863".into(),
        name: "TestBot".into(),
        bot: None,
    }));

    let res_31 = run_preflight(&mut config, &reporter, &checker).await;
    assert!(res_31.is_err());

    config.api.secret = "a".repeat(32);
    let res_32 = run_preflight(&mut config, &reporter, &checker).await;
    assert!(res_32.is_ok());
}

#[tokio::test]
async fn test_preflight_guild_id_proxy_cookies_rules() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let reporter = Reporter::new(true);

    let mut map = HashMap::new();
    map.insert("DISCORD_TOKEN".into(), valid_token_string());
    map.insert("API_SECRET".into(), "a".repeat(32));

    let (checker, _) = MockTokenChecker::new(TokenCheckResult::Ok(ApplicationInfo {
        id: "1011718919473610863".into(),
        name: "TestBot".into(),
        bot: None,
    }));

    // Rule e: invalid GUILD_ID
    let mut config = Config::from_map(root.clone(), &map);
    config.discord.guild_id = Some("invalid_guild".into());
    assert!(run_preflight(&mut config, &reporter, &checker)
        .await
        .is_err());
    config.discord.guild_id = Some("1011718919473610863".into());
    assert!(run_preflight(&mut config, &reporter, &checker)
        .await
        .is_ok());

    // Rule f: invalid PROXY_URL
    config.proxies = vec!["invalid-proxy".into()];
    assert!(run_preflight(&mut config, &reporter, &checker)
        .await
        .is_err());
    config.proxies = vec!["http://127.0.0.1:8080".into()];
    assert!(run_preflight(&mut config, &reporter, &checker)
        .await
        .is_ok());

    // Rule g: missing COOKIES_FILE
    config.ytdlp.cookies_file = "non_existent_cookies.txt".into();
    assert!(run_preflight(&mut config, &reporter, &checker)
        .await
        .is_err());
    config.ytdlp.cookies_file = String::new();

    // Rule g: unsupported COOKIES_FROM_BROWSER
    config.ytdlp.cookies_from_browser = "unsupported_browser:default".into();
    assert!(run_preflight(&mut config, &reporter, &checker)
        .await
        .is_err());
    config.ytdlp.cookies_from_browser = "firefox:default".into();
    assert!(run_preflight(&mut config, &reporter, &checker)
        .await
        .is_ok());
}
