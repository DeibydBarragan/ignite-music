use regex::Regex;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;
use url::Url;

pub const MIN_API_SECRET_LENGTH: usize = 32;
pub const BROWSERS: &[&str] = &[
    "brave", "chrome", "chromium", "edge", "firefox", "opera", "safari", "vivaldi", "whale",
];
pub const TOKEN_STEPS: &[&str] = &[
    "1. Open https://discord.com/developers/applications and select your bot",
    "2. Go to \"Bot\" and press \"Reset Token\", then copy the token",
    "3. Paste it into the .env file in the bot folder:  DISCORD_TOKEN=your_token",
];

static SNOWFLAKE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{17,20}$").unwrap());
static TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\w-]{20,}\.[\w-]{4,}\.[\w-]{20,}$").unwrap());

pub fn is_snowflake(val: &str) -> bool {
    SNOWFLAKE.is_match(val)
}

pub fn is_token_format(val: &str) -> bool {
    TOKEN.is_match(val)
}

pub fn validate_proxy(addr: &str) -> bool {
    if let Ok(u) = Url::parse(addr) {
        let scheme = u.scheme();
        let valid_scheme = matches!(scheme, "http" | "https" | "socks4" | "socks5" | "socks5h");
        valid_scheme && u.host().is_some()
    } else {
        false
    }
}

pub fn can_write(dir: &Path) -> bool {
    if fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(format!(".write-test-{:x}", rand_u32()));
    if fs::write(&probe, "ok").is_err() {
        return false;
    }
    let _ = fs::remove_file(&probe);
    true
}

fn rand_u32() -> u32 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    (now.as_nanos() & 0xFFFFFFFF) as u32
}

pub fn ensure_env_file(root: &Path, token_env: Option<&str>) -> bool {
    let env_file = root.join(".env");
    let example = root.join(".env.example");
    let token_present = token_env.is_some_and(|t| !t.is_empty());
    if env_file.exists() || token_present {
        return false;
    }
    if !example.exists() {
        return false;
    }
    fs::copy(&example, &env_file).is_ok()
}
