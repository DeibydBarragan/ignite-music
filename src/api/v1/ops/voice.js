'use strict';

/**
 * Voice settings and dev logs API operations.
 * Registered on MusicManager's apiOps registry to run inside the shard that owns the guild.
 */

const { voiceSettingsStore } = require('../../../voice/settings-store');
const { voiceEventLogStore } = require('../../../voice/event-log');
const { ApiError } = require('../../contracts');
const { voiceReceiver } = require('../../../voice');

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

function getVoiceLogs(_music, { guildId, limit }) {
    return { logs: voiceEventLogStore.getLogs(guildId, limit) };
}

function clearVoiceLogs(_music, { guildId }) {
    voiceEventLogStore.clear(guildId);
    return { ok: true };
}

async function joinVoice(music, { guildId, channelId = null, userId = null }) {
    const guild = music.client.guilds.cache.get(guildId)
        || await music.client.guilds.fetch(guildId).catch(() => null);
    if (!guild) throw new ApiError(404, 'guild_not_found', 'Guild not found');

    let channel = null;

    if (channelId) {
        channel = guild.channels.cache.get(channelId)
            || await guild.channels.fetch(channelId).catch(() => null);
    }

    if (!channel && userId) {
        const member = guild.members.cache.get(userId)
            || await guild.members.fetch(userId).catch(() => null);
        channel = member?.voice?.channel || null;
    }

    if (!channel) {
        channel = guild.channels.cache
            .filter((c) => c.isVoiceBased() && c.members.some((m) => !m.user.bot))
            .first() || null;
    }

    if (!channel) {
        channel = guild.channels.cache.filter((c) => c.isVoiceBased()).first() || null;
    }

    if (!channel) {
        throw new ApiError(400, 'no_voice_channel', '¡No se encontró ningún canal de voz disponible!');
    }

    const me = guild.members.me || await guild.members.fetch(music.client.user.id).catch(() => null);
    if (me && typeof channel.permissionsFor === 'function') {
        const perms = channel.permissionsFor(me);
        if (!perms?.has('Connect')) {
            throw new ApiError(403, 'voice_permission_denied', `No tengo permisos para unirme a "${channel.name}".`);
        }
    }

    music.state(guildId).disconnectReason = null;
    const voice = await music.distube.voices.join(channel);
    const connection = voice?.connection;
    if (connection) {
        try {
            if (typeof voice.setSelfDeaf === 'function') voice.setSelfDeaf(false);
            else if (typeof connection.rejoin === 'function') connection.rejoin({ ...connection.joinConfig, selfDeaf: false });
        } catch {}
        try {
            if (me?.voice?.serverDeaf && typeof me.voice.setDeaf === 'function') {
                await me.voice.setDeaf(false, 'Voice commands enabled');
            }
        } catch {}
        voiceReceiver.attach(guildId, connection, { client: music.client });
    }

    return {
        success: true,
        channel: { id: channel.id, name: channel.name },
        message: `¡Conectado al canal ${channel.name}!`,
    };
}

async function leaveVoice(music, { guildId }) {
    const guild = music.client.guilds.cache.get(guildId)
        || await music.client.guilds.fetch(guildId).catch(() => null);
    if (!guild) throw new ApiError(404, 'guild_not_found', 'Guild not found');

    const queue = music.distube.getQueue(guildId);
    if (queue) {
        await queue.stop().catch(() => undefined);
    }
    music.state(guildId).disconnectReason = 'user_leave';
    music.distube.voices.leave(guildId);
    return {
        success: true,
        message: 'Desconectado del canal de voz',
    };
}

function register(ops) {
    ops.register('voice.get', (music, payload) => getVoiceSettings(music, payload));
    ops.register('voice.update', (music, payload) => updateVoiceSettings(music, payload));
    ops.register('voice.reset', (music, payload) => resetVoiceSettings(music, payload));
    ops.register('voice.logs', (music, payload) => getVoiceLogs(music, payload));
    ops.register('voice.clearLogs', (music, payload) => clearVoiceLogs(music, payload));
    ops.register('voice.join', (music, payload) => joinVoice(music, payload));
    ops.register('voice.leave', (music, payload) => leaveVoice(music, payload));
}

module.exports = {
    register,
    getVoiceSettings,
    updateVoiceSettings,
    resetVoiceSettings,
    getVoiceLogs,
    clearVoiceLogs,
    joinVoice,
    leaveVoice,
};
