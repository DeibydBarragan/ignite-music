#![allow(clippy::print_stdout, clippy::print_stderr)]

use crate::config::Config;
use crate::error::ArranqueError;
pub use crate::preflight_rules::*;
use crate::report::Reporter;
use crate::token::{TokenCheckResult, TokenChecker};

pub async fn run_preflight<T: TokenChecker>(
    config: &mut Config,
    reporter: &Reporter,
    token_checker: &T,
) -> Result<(), ArranqueError> {
    reporter.title("Ignite startup check");
    let mut problems = 0;

    let has_env_token = std::env::var("DISCORD_TOKEN")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    if ensure_env_file(&config.root, if has_env_token { Some("set") } else { None }) {
        reporter.fail(
            "A .env file was created for you, but it still needs your bot token",
            TOKEN_STEPS,
        );
        return Err(reporter.stop(1));
    }

    if config.discord.token.is_empty() {
        problems += 1;
        reporter.fail("DISCORD_TOKEN is empty", TOKEN_STEPS);
    } else if !is_token_format(&config.discord.token) {
        problems += 1;
        let mut lines = vec![
            "Copy the token again without spaces or quotes. It looks like: MTA1...xyz.Gh7k2a.abc...",
        ];
        lines.extend_from_slice(TOKEN_STEPS);
        reporter.fail("DISCORD_TOKEN does not look like a bot token", &lines);
    } else {
        match token_checker.check(&config.discord.token).await {
            TokenCheckResult::Ok(app) => {
                let name = app.display_name().to_string();
                if !config.discord.client_id.is_empty() && config.discord.client_id != app.id {
                    reporter.warn(
                        &format!(
                            "CLIENT_ID in .env ({}) belongs to another application",
                            config.discord.client_id
                        ),
                        &[&format!(
                            "Using this bot's own id instead: {}. You can remove CLIENT_ID from .env.",
                            app.id
                        )],
                    );
                }
                config.discord.client_id = app.id.clone();
                reporter.ok("Discord token", &format!("{name} (application {})", app.id));
            }
            TokenCheckResult::Invalid => {
                problems += 1;
                reporter.fail(
                    "Discord rejected DISCORD_TOKEN (wrong or reset token)",
                    TOKEN_STEPS,
                );
            }
            TokenCheckResult::Unreachable(detail) => {
                if config.discord.client_id.is_empty() {
                    problems += 1;
                    reporter.fail(
                        "Could not reach Discord to check the token",
                        &[
                            &format!("Reason: {detail}"),
                            "Check the internet connection of this machine (and firewall / proxy settings).",
                        ],
                    );
                } else {
                    reporter.warn(
                        "Could not reach Discord to check the token, trying anyway",
                        &[&format!("Reason: {detail}")],
                    );
                }
            }
        }
    }

    if let Some(guild) = &config.discord.guild_id {
        if !is_snowflake(guild) {
            problems += 1;
            reporter.fail(
                &format!("GUILD_ID \"{guild}\" is not a server id"),
                &[
                    "Leave GUILD_ID empty to register commands everywhere, or put a server id (Developer Mode > right click the server > Copy Server ID).",
                ],
            );
        }
    }

    if config.sharding.total_shards != "auto" && config.sharding.total_shards != "1" {
        problems += 1;
        reporter.fail(
            "TOTAL_SHARDS is invalid",
            &["TOTAL_SHARDS must be 'auto' or '1' in Rust."],
        );
    }

    for proxy in &config.proxies {
        if !validate_proxy(proxy) {
            problems += 1;
            reporter.fail(
                &format!("PROXY_URL \"{proxy}\" is not a valid proxy address"),
                &["Use the form http://user:password@host:port (several can be separated by commas)."],
            );
        }
    }

    if !config.ytdlp.cookies_file.is_empty() {
        let file = config.root.join(&config.ytdlp.cookies_file);
        if !file.exists() {
            problems += 1;
            reporter.fail(
                &format!("COOKIES_FILE \"{}\" does not exist", config.ytdlp.cookies_file),
                &[
                    &format!("Expected at: {}", file.display()),
                    "Export your YouTube cookies to that file (see README.md, \"YouTube: Sign in to confirm you're not a bot\"),",
                    "or remove COOKIES_FILE from .env.",
                ],
            );
        } else {
            let canon = file.canonicalize().unwrap_or(file);
            config.ytdlp.cookies_file = canon.to_string_lossy().to_string();
            reporter.ok("YouTube cookies", &config.ytdlp.cookies_file);
        }
    } else if !config.ytdlp.cookies_from_browser.is_empty() {
        let b = config
            .ytdlp
            .cookies_from_browser
            .split(':')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if !BROWSERS.contains(&b.as_str()) {
            problems += 1;
            reporter.fail(
                &format!(
                    "COOKIES_FROM_BROWSER \"{}\" is not a supported browser",
                    config.ytdlp.cookies_from_browser
                ),
                &[&format!("Use one of: {}", BROWSERS.join(", "))],
            );
        } else {
            reporter.ok("YouTube cookies", &format!("from {b}"));
        }
    }

    if !can_write(&config.data_dir) {
        problems += 1;
        reporter.fail(
            &format!("Cannot write to {}", config.data_dir.display()),
            &["The bot saves settings there. Give your user write permission to the bot folder."],
        );
    }

    if config.api.enabled {
        if config.api.secret.len() < MIN_API_SECRET_LENGTH {
            problems += 1;
            reporter.fail(
                &format!("API_SECRET is missing or too short (minimum {MIN_API_SECRET_LENGTH} characters)"),
                &[
                    "The REST API refuses to start with a weak, empty or default secret.",
                    "Generate one and add it to .env:",
                    "  node -e \"console.log(require('crypto').randomBytes(32).toString('hex'))\"",
                    "Or set API_ENABLED=false to run the bot without the web dashboard API.",
                ],
            );
        } else {
            reporter.ok(
                "API secret",
                &format!("configured ({MIN_API_SECRET_LENGTH}+ characters)"),
            );
        }
        if config.api.allowed_origins.is_empty() {
            reporter.info("API_ALLOWED_ORIGINS is empty", "browser cross-origin requests will be rejected (server-to-server clients are unaffected)");
        }
    }

    if config.spotify.client_id.is_empty() || config.spotify.client_secret.is_empty() {
        reporter.info(
            "Spotify API keys not set",
            "optional: large Spotify playlists load only their first 100 songs",
        );
    }

    if problems > 0 {
        Err(reporter.stop(problems))
    } else {
        println!();
        Ok(())
    }
}
