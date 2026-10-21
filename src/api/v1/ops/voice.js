'use strict';

/**
 * Voice settings API operations.
 * Registered on MusicManager's apiOps registry to run inside the shard that owns the guild.
 */

const { voiceSettingsStore } = require('../../../voice/settings-store');

async function getVoiceSettings(_music, { guildId }) {
    const settings = await voiceSettingsStore.getGuildSettings(guildId);
    return settings;
}

async function updateVoiceSettings(_music, payload) {
    const { guildId, ...data } = payload;
    const updated = await voiceSettingsStore.updateGuildSettings(guildId, data);
    return updated;
}

async function resetVoiceSettings(_music, { guildId }) {
    const reset = await voiceSettingsStore.resetGuildSettings(guildId);
    return reset;
}

function register(ops) {
    ops.register('voice.get', (music, payload) => getVoiceSettings(music, payload));
    ops.register('voice.update', (music, payload) => updateVoiceSettings(music, payload));
    ops.register('voice.reset', (music, payload) => resetVoiceSettings(music, payload));
}

module.exports = {
    register,
    getVoiceSettings,
    updateVoiceSettings,
    resetVoiceSettings,
};
