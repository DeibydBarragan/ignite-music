'use strict';

/**
 * Decides whether audio from a Discord user may be processed.
 * Rejects the bot itself and other bots (music bots, TTS voices) so their audio
 * can never trigger commands, unless bot input is explicitly allowed for tests.
 */

const config = require('../config');
const log = require('../core/logger').createLogger('voice-speaker');

/**
 * @param {object} params
 * @param {import('discord.js').Client|null} params.client
 * @param {string} params.guildId
 * @param {string} params.userId
 * @param {boolean} [params.allowBotInput=false] - Test override that accepts any bot.
 * @returns {Promise<boolean>} True when this speaker should be listened to.
 */
async function shouldListenTo({ client, guildId, userId, allowBotInput = false }) {
    // Filter out self (prevent infinite feedback loops)
    if (client?.user?.id && userId === client.user.id) {
        log.debug({ guildId, userId }, 'Ignoring audio from bot itself');
        return false;
    }

    if (!client) return true;

    try {
        const guild = client.guilds.cache.get(guildId) || await client.guilds.fetch(guildId).catch(() => null);
        const member = guild?.members.cache.get(userId) || await guild?.members.fetch(userId).catch(() => null);
        if (member?.user?.bot) {
            const isSoundBot = Boolean(config.voice.soundBotId && String(userId) === String(config.voice.soundBotId));
            const allowInput = Boolean(allowBotInput || (config.voice.allowBotInput && isSoundBot));
            if (!allowInput) {
                log.info({ guildId, userId }, 'Ignoring audio from bot member');
                return false;
            }
            log.info({ guildId, userId }, 'Accepting audio from sound bot for recognition');
        }
    } catch {
        // Member lookup failed: still refuse users the client already knows are bots
        if (client.users?.cache?.get(userId)?.bot && !allowBotInput) {
            log.info({ guildId, userId }, 'Ignoring audio from cached bot user');
            return false;
        }
    }

    return true;
}

module.exports = {
    shouldListenTo,
};
