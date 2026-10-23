'use strict';

/**
 * Voice Command Dispatcher:
 * Connects the voice matcher to DisTube playback and controls,
 * and triggers auditory confirmation feedback.
 */

const log = require('../core/logger').createLogger('voice-dispatcher');
const { applyControl } = require('../music/actions');
const { matchVoiceCommand } = require('./matcher');
const { sendTtsFeedback } = require('./tts-feedback');
const { voiceSettingsStore } = require('./settings-store');

/**
 * Executes a transcribed voice input against the MusicManager.
 *
 * @param {import('../music/manager').MusicManager} manager - Active MusicManager instance.
 * @param {object} payload
 * @param {string} payload.guildId - Guild snowflake.
 * @param {string} payload.userId - User who spoke.
 * @param {string} payload.text - Transcribed text.
 * @param {string} [payload.channelId] - Voice channel ID where user spoke.
 * @param {object} [options]
 * @param {object} [options.settings] - Guild voice settings (wake_word, tts_feedback_enabled, etc.).
 * @returns {Promise<{ executed: boolean, commandId?: string, query?: string, reason?: string, result?: any }>}
 */
async function dispatchVoiceCommand(manager, payload, options = {}) {
    const { guildId, userId, text, channelId } = payload;
    const settings = options.settings || await voiceSettingsStore.getGuildSettings(guildId);

    if (settings && settings.enabled === false) {
        log.debug({ guildId }, 'Voice commands are disabled for this guild');
        return { executed: false, reason: 'voice_disabled_for_guild' };
    }

    // 1. Match voice input
    const match = matchVoiceCommand(text, {
        wakeWord: settings?.wake_word,
        customBindings: settings?.bindings,
    });

    if (!match.matched) {
        log.debug({ guildId, userId, reason: match.reason }, 'Voice input did not match any command');
        return { executed: false, reason: match.reason };
    }

    const { commandId, query, missingQuery } = match;

    if (missingQuery) {
        const queue = manager.getQueue(guildId);
        const t = manager.translator(guildId);
        // If play/reproduce was invoked without arguments while paused, seamlessly resume
        if (commandId === 'play' && queue && (queue.paused || !queue.playing)) {
            log.info({ guildId, userId }, 'Play invoked without argument while paused; resuming playback');
            const result = await applyControl(queue, t, 'resume', null, manager);
            const voiceChannelId = channelId || queue.voiceChannel?.id;
            if (voiceChannelId) {
                sendTtsFeedback({
                    guildId,
                    channelId: voiceChannelId,
                    userId,
                    commandId: 'resume',
                    params: {},
                }, {
                    enabled: settings?.tts_feedback_enabled,
                    templates: settings?.tts_templates,
                }).catch(() => undefined);
            }
            return { executed: result.ok, commandId: 'resume', result };
        }
        log.info({ guildId, userId, commandId }, 'Voice command missing required parameter');
        return { executed: false, reason: 'missing_argument', commandId };
    }

    log.info({ guildId, userId, commandId, query }, 'Executing voice command');

    // 2. Resolve Discord guild and member
    const guild = manager.client.guilds.cache.get(guildId)
        || await manager.client.guilds.fetch(guildId).catch(() => null);
    if (!guild) {
        return { executed: false, reason: 'guild_not_found' };
    }

    const queue = manager.getQueue(guildId);
    const t = manager.translator(guildId);

    const member = guild.members.cache.get(userId)
        || await guild.members.fetch(userId).catch(() => null);
    const voiceChannel = member?.voice?.channel
        || queue?.voiceChannel
        || (channelId ? guild.channels.cache.get(channelId) : null);

    let executionResult = null;

    try {
        switch (commandId) {
            case 'play':
            case 'enqueue': {
                if (!voiceChannel) {
                    return { executed: false, reason: 'user_not_in_voice' };
                }
                // Trigger auditory TTS confirmation immediately before resolving song
                const voiceChannelId = voiceChannel?.id || channelId;
                if (voiceChannelId) {
                    sendTtsFeedback({
                        guildId,
                        channelId: voiceChannelId,
                        userId,
                        commandId,
                        params: { query },
                    }, {
                        enabled: settings?.tts_feedback_enabled,
                        templates: settings?.tts_templates,
                    }).catch(() => undefined);
                }
                const textChannel = queue?.textChannel || null;
                executionResult = await manager.play({
                    voiceChannel,
                    textChannel,
                    member,
                    query,
                    skip: false,
                });
                break;
            }

            case 'pause':
            case 'resume':
            case 'skip':
            case 'previous':
            case 'stop':
            case 'shuffle':
            case 'volume': {
                if (!queue) {
                    return { executed: false, reason: 'no_active_queue' };
                }
                const value = commandId === 'volume' ? Number.parseInt(query, 10) : undefined;
                executionResult = await applyControl(queue, t, commandId, value, manager);
                break;
            }

            case 'queue': {
                if (!queue) {
                    return { executed: false, reason: 'no_active_queue' };
                }
                executionResult = { ok: true, count: queue.songs.length };
                break;
            }

            default:
                return { executed: false, reason: 'unsupported_command', commandId };
        }

        // 3. Trigger auditory TTS confirmation via ignite-sound (for non-play commands)
        if (commandId !== 'play' && commandId !== 'enqueue') {
            const voiceChannelId = voiceChannel?.id || channelId;
            if (voiceChannelId) {
                sendTtsFeedback({
                    guildId,
                    channelId: voiceChannelId,
                    userId,
                    commandId,
                    params: { query },
                }, {
                    enabled: settings?.tts_feedback_enabled,
                    templates: settings?.tts_templates,
                }).catch(() => undefined);
            }
        }

        return {
            executed: true,
            commandId,
            query,
            result: executionResult,
        };
    } catch (err) {
        log.error({ err, guildId, commandId }, 'Failed to execute matched voice command');
        return { executed: false, reason: 'execution_error', error: err.message };
    }
}

module.exports = {
    dispatchVoiceCommand,
};
