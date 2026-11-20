//! Voice stream receiver and utterance segmenter (Spec S-08).
//!
//! Converts incoming Songbird voice ticks into complete speech fragments
//! segmented by user, applying Voice Activity Detection (silence cutoff),
//! minimum duration filtering, and maximum duration clipping.

use crate::speakers::SpeakerFilter;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{debug, info};

pub const TICK_MS: u32 = 20;
pub const SAMPLES_PER_MS_48K_STEREO: usize = 96; // 48kHz * 2 channels / 1000ms

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceFragment {
    pub guild_id: u64,
    pub user_id: u64,
    /// 48 kHz 16-bit stereo PCM
    pub pcm48k: Vec<i16>,
    pub duration_ms: u32,
}

#[derive(Debug, Clone)]
pub struct ReceiverConfig {
    pub silence_duration_ms: u32,
    pub min_duration_ms: u32,
    pub max_duration_ms: u32,
}

impl Default for ReceiverConfig {
    fn default() -> Self {
        Self {
            silence_duration_ms: 500,
            min_duration_ms: 650,
            max_duration_ms: 12_000,
        }
    }
}

struct UserBuffer {
    pcm48k: Vec<i16>,
    voice_ms: u32,
    silence_ms: u32,
}

pub struct VoiceSegmenter {
    pub guild_id: u64,
    pub config: ReceiverConfig,
    speaker_filter: Arc<SpeakerFilter>,
    ssrc_to_user: HashMap<u32, u64>,
    active_buffers: HashMap<u64, UserBuffer>,
}

impl VoiceSegmenter {
    pub fn new(guild_id: u64, config: ReceiverConfig, filter: Arc<SpeakerFilter>) -> Self {
        Self {
            guild_id,
            config,
            speaker_filter: filter,
            ssrc_to_user: HashMap::new(),
            active_buffers: HashMap::new(),
        }
    }

    pub fn register_ssrc(&mut self, ssrc: u32, user_id: Option<u64>) {
        if let Some(uid) = user_id {
            self.ssrc_to_user.insert(ssrc, uid);
        }
    }

    pub async fn process_tick(
        &mut self,
        speaking: &[(u32, &[i16])],
        silent: &[u32],
    ) -> Vec<VoiceFragment> {
        let mut completed = Vec::new();
        let mut speaking_users = HashSet::new();

        for &(ssrc, pcm) in speaking {
            let Some(&user_id) = self.ssrc_to_user.get(&ssrc) else {
                continue;
            };

            if !self
                .speaker_filter
                .should_listen_to(self.guild_id, user_id, false)
                .await
            {
                continue;
            }

            speaking_users.insert(user_id);
            let buf = self
                .active_buffers
                .entry(user_id)
                .or_insert_with(|| UserBuffer {
                    pcm48k: Vec::new(),
                    voice_ms: 0,
                    silence_ms: 0,
                });

            buf.pcm48k.extend_from_slice(pcm);
            let frame_ms = (pcm.len() / SAMPLES_PER_MS_48K_STEREO) as u32;
            buf.voice_ms += frame_ms;
            buf.silence_ms = 0;

            // Rule 5: 12,000 ms cut and process
            if buf.voice_ms >= self.config.max_duration_ms {
                info!(
                    user_id,
                    duration_ms = buf.voice_ms,
                    "Reached maximum voice duration; cutting fragment"
                );
                completed.push(VoiceFragment {
                    guild_id: self.guild_id,
                    user_id,
                    pcm48k: std::mem::take(&mut buf.pcm48k),
                    duration_ms: buf.voice_ms,
                });
                self.active_buffers.remove(&user_id);
            }
        }

        // Songbird silent list and inactive users both contribute to silence ticks
        let silent_users: HashSet<u64> = silent
            .iter()
            .filter_map(|ssrc| self.ssrc_to_user.get(ssrc).copied())
            .collect();

        // Ticks of silence for active users not speaking or explicitly reported silent
        let mut finished_users = Vec::new();
        for (&user_id, buf) in self.active_buffers.iter_mut() {
            if silent_users.contains(&user_id) || !speaking_users.contains(&user_id) {
                buf.silence_ms += TICK_MS;
                if buf.silence_ms >= self.config.silence_duration_ms {
                    finished_users.push(user_id);
                }
            }
        }

        for user_id in finished_users {
            if let Some(buf) = self.active_buffers.remove(&user_id) {
                // Rule 4: Discard if < 650 ms
                if buf.voice_ms >= self.config.min_duration_ms {
                    completed.push(VoiceFragment {
                        guild_id: self.guild_id,
                        user_id,
                        pcm48k: buf.pcm48k,
                        duration_ms: buf.voice_ms,
                    });
                } else {
                    debug!(
                        user_id,
                        duration_ms = buf.voice_ms,
                        "Voice fragment too short; discarded"
                    );
                }
            }
        }

        completed
    }

    pub fn on_client_disconnect(&mut self, user_id: u64) -> Option<VoiceFragment> {
        self.ssrc_to_user.retain(|_, user| *user != user_id);
        if let Some(buf) = self.active_buffers.remove(&user_id) {
            if buf.voice_ms >= self.config.min_duration_ms {
                return Some(VoiceFragment {
                    guild_id: self.guild_id,
                    user_id,
                    pcm48k: buf.pcm48k,
                    duration_ms: buf.voice_ms,
                });
            }
        }
        None
    }
}

mod handler;
pub use handler::VoiceReceiverHandler;
