//! Speaker filter for voice recognition (Spec S-08).
//!
//! Rejects audio from the bot itself and other bots (music bots, TTS) to
//! prevent infinite feedback loops and accidental triggers, unless bot input
//! is explicitly allowed (e.g. for testing with sound bot).

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, info};
use twilight_http::Client as TwilightClient;
use twilight_model::id::{
    marker::{GuildMarker, UserMarker},
    Id,
};

#[derive(Debug, Clone, Default)]
pub struct SpeakerFilterConfig {
    pub bot_id: Option<u64>,
    pub sound_bot_id: Option<u64>,
    pub allow_bot_input: bool,
}

pub struct SpeakerFilter {
    config: SpeakerFilterConfig,
    client: Option<Arc<TwilightClient>>,
    bot_cache: RwLock<HashMap<u64, bool>>,
}

impl SpeakerFilter {
    pub fn new(config: SpeakerFilterConfig, client: Option<Arc<TwilightClient>>) -> Self {
        Self {
            config,
            client,
            bot_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Pre-populates known bot or human users in cache (useful for tests).
    pub fn set_known_user(&self, user_id: u64, is_bot: bool) {
        let mut cache = self.bot_cache.write().unwrap();
        cache.insert(user_id, is_bot);
    }

    pub fn set_bot_id(&mut self, bot_id: u64) {
        self.config.bot_id = Some(bot_id);
    }

    /// Evaluates whether incoming audio from `user_id` in `guild_id` should be processed.
    pub async fn should_listen_to(
        &self,
        guild_id: u64,
        user_id: u64,
        allow_bot_override: bool,
    ) -> bool {
        // 1. Ignore audio from the bot itself
        if let Some(bot_id) = self.config.bot_id {
            if user_id == bot_id {
                debug!(guild_id, user_id, "Ignoring audio from bot itself");
                return false;
            }
        }

        // 2. Check cached bot status
        let cached = {
            let cache = self.bot_cache.read().unwrap();
            cache.get(&user_id).copied()
        };

        let is_bot = match cached {
            Some(bot) => bot,
            None => {
                let bot = if let Some(client) = &self.client {
                    let guild_marker = Id::<GuildMarker>::new(guild_id);
                    let user_marker = Id::<UserMarker>::new(user_id);
                    let Ok(Some(member)) =
                        tokio::time::timeout(std::time::Duration::from_secs(1), async {
                            let response =
                                client.guild_member(guild_marker, user_marker).await.ok()?;
                            response.model().await.ok()
                        })
                        .await
                    else {
                        return false;
                    };
                    member.user.bot
                } else {
                    false
                };
                self.bot_cache.write().unwrap().insert(user_id, bot);
                bot
            }
        };

        if is_bot {
            let is_sound_bot = self.config.sound_bot_id == Some(user_id);
            let allow = allow_bot_override || (self.config.allow_bot_input && is_sound_bot);
            if !allow {
                info!(guild_id, user_id, "Ignoring audio from bot member");
                return false;
            }
            info!(
                guild_id,
                user_id, "Accepting audio from sound bot for recognition"
            );
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_speaker_filter_rules() {
        let config = SpeakerFilterConfig {
            bot_id: Some(100),
            sound_bot_id: Some(200),
            allow_bot_input: true,
        };
        let filter = SpeakerFilter::new(config, None);
        filter.set_known_user(200, true); // sound bot
        filter.set_known_user(300, true); // random bot
        filter.set_known_user(400, false); // human

        // Bot itself is always rejected
        assert!(!filter.should_listen_to(1, 100, false).await);
        assert!(!filter.should_listen_to(1, 100, true).await);

        // Sound bot is accepted with allow_bot_input
        assert!(filter.should_listen_to(1, 200, false).await);

        // Random bot rejected without override
        assert!(!filter.should_listen_to(1, 300, false).await);
        // Random bot accepted with override
        assert!(filter.should_listen_to(1, 300, true).await);

        // Human accepted
        assert!(filter.should_listen_to(1, 400, false).await);
    }
}
