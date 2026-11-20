use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationBot {
    pub username: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationInfo {
    pub id: String,
    pub name: String,
    pub bot: Option<ApplicationBot>,
}

impl ApplicationInfo {
    pub fn display_name(&self) -> &str {
        if let Some(bot) = &self.bot {
            if let Some(u) = &bot.username {
                if !u.is_empty() {
                    return u.as_str();
                }
            }
        }
        self.name.as_str()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TokenCheckResult {
    Ok(ApplicationInfo),
    Invalid,
    Unreachable(String),
}

pub async fn check_token_with_url(
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
) -> TokenCheckResult {
    let req = client
        .get(endpoint)
        .header("Authorization", format!("Bot {token}"))
        .timeout(Duration::from_secs(10));

    match req.send().await {
        Ok(resp) => {
            if resp.status().as_u16() == 401 {
                TokenCheckResult::Invalid
            } else if !resp.status().is_success() {
                TokenCheckResult::Unreachable(format!("HTTP {}", resp.status().as_u16()))
            } else {
                match resp.json::<ApplicationInfo>().await {
                    Ok(app) => TokenCheckResult::Ok(app),
                    Err(e) => TokenCheckResult::Unreachable(e.to_string()),
                }
            }
        }
        Err(e) => TokenCheckResult::Unreachable(e.to_string()),
    }
}

pub async fn check_token(token: &str) -> TokenCheckResult {
    let client = reqwest::Client::new();
    check_token_with_url(
        &client,
        "https://discord.com/api/v10/applications/@me",
        token,
    )
    .await
}

pub trait TokenChecker: Send + Sync {
    fn check<'a>(
        &'a self,
        token: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = TokenCheckResult> + Send + 'a>>;
}

pub struct DiscordTokenChecker;

impl TokenChecker for DiscordTokenChecker {
    fn check<'a>(
        &'a self,
        token: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = TokenCheckResult> + Send + 'a>> {
        Box::pin(check_token(token))
    }
}
