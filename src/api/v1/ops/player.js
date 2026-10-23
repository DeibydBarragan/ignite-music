'use strict';

/**
 * Player operations. These run inside the shard that owns the guild (see
 * src/api/shard-proxy.js) and return JSON-serializable bodies that match the
 * shapes the Next.js dashboard already consumes.
 */

const { RepeatMode } = require('distube');
const config = require('../../../config');
const { unsupportedUrlReason } = require('../../../core/youtube-guards');
const { ApiError, serializeTrack } = require('../../contracts');
const { applyControl, CONTROL_ACTIONS, ensureResumed } = require('../../../music/actions');

function state(music, { guildId }) {
    const queue = music.distube.getQueue(guildId);
    const musicState = music.state(guildId);
    const disconnectReason = musicState?.disconnectReason || null;

    if (!queue) {
        return {
            hasQueue: false,
            isPlaying: false,
            isPaused: false,
            currentTime: 0,
            duration: 0,
            volume: config.bot.defaultVolume,
            repeatMode: 'off',
            autoplay: false,
            filter: null,
            currentSong: null,
            queue: [],
            disconnectReason,
        };
    }

    const currentSong = queue.songs[0] ? serializeTrack(queue.songs[0]) : null;
    const upcoming = queue.songs.slice(1).map((song) => serializeTrack(song));

    let repeatMode = 'off';
    if (queue.repeatMode === RepeatMode.SONG) repeatMode = 'track';
    else if (queue.repeatMode === RepeatMode.QUEUE) repeatMode = 'queue';

    return {
        hasQueue: true,
        isPlaying: !queue.paused && queue.playing,
        isPaused: Boolean(queue.paused),
        currentTime: Math.floor(queue.currentTime || 0),
        duration: currentSong?.duration || 0,
        volume: queue.volume,
        repeatMode,
        autoplay: Boolean(queue.autoplay),
        filter: queue.filters.names[0] || null,
        voiceChannel: queue.voiceChannel ? { id: queue.voiceChannel.id, name: queue.voiceChannel.name } : null,
        currentSong,
        queue: upcoming,
        disconnectReason,
    };
}

/**
 * Resolve the acting member (with a lightweight fallback for users that are
 * not cached in the guild) and validate voice-channel constraints.
 */
async function resolveVoiceChannel(music, guild, { voiceChannelId, userId }) {
    const client = music.client;
    const botVoiceChannel = guild.members.me?.voice?.channel;
    const wrongChannel = () => new ApiError(
        400,
        'WRONG_VOICE_CHANNEL',
        `¡Debes estar en el mismo canal de voz que el bot (${botVoiceChannel.name})!`,
    );
    const notInVoice = () => new ApiError(
        400,
        'NOT_IN_VOICE',
        '¡Debes estar en un canal de voz en Discord para reproducir música!',
    );

    let channel = null;
    let member = null;

    if (userId) {
        member = guild.members.cache.get(userId) || await guild.members.fetch(userId).catch(() => null);
        const userVoice = member?.voice?.channel;
        if (userVoice) {
            if (botVoiceChannel && botVoiceChannel.id !== userVoice.id) throw wrongChannel();
            channel = userVoice;
        } else if (voiceChannelId) {
            channel = guild.channels.cache.get(voiceChannelId) || await guild.channels.fetch(voiceChannelId).catch(() => null);
        } else {
            throw notInVoice();
        }
    } else if (voiceChannelId) {
        channel = guild.channels.cache.get(voiceChannelId) || await guild.channels.fetch(voiceChannelId).catch(() => null);
        if (botVoiceChannel && channel && botVoiceChannel.id !== channel.id) throw wrongChannel();
    }

    const existingQueue = music.distube.getQueue(guild.id);
    if (!channel && existingQueue) channel = existingQueue.voiceChannel;
    if (!channel) throw notInVoice();

    if (!member && userId) {
        const user = client.users.cache.get(userId) || await client.users.fetch(userId).catch(() => null);
        if (user) {
            member = {
                id: user.id,
                user,
                displayName: user.globalName || user.username,
                guild,
                displayAvatarURL: (opts) => (typeof user.displayAvatarURL === 'function' ? user.displayAvatarURL(opts) : null),
            };
        }
    }

    return { channel, member };
}

async function play(music, { guildId, query, voiceChannelId = null, userId = null, next = false, skip = false, devMode = false }) {
    const guild = music.client.guilds.cache.get(guildId);
    if (!guild) throw new ApiError(404, 'guild_not_found', 'Guild not found');
    const unsupported = unsupportedUrlReason(query);
    if (unsupported) throw new ApiError(400, 'UNSUPPORTED_URL', unsupported);

    const { channel, member } = await resolveVoiceChannel(music, guild, { voiceChannelId, userId });

    const outcome = await music.play({
        voiceChannel: channel,
        member: member || guild.members.me,
        query,
        position: next ? 1 : 0,
        skip: Boolean(skip),
        devMode: Boolean(devMode),
    });

    const queue = music.distube.getQueue(guildId);
    if (skip) await ensureResumed(queue);

    const resolved = outcome?.resolved;
    const isPlaylist = resolved && Array.isArray(resolved.songs);
    return {
        success: true,
        started: Boolean(outcome?.started),
        position: outcome?.position || 0,
        song: resolved ? serializeTrack(isPlaylist ? resolved.songs[0] : resolved) : null,
        playlist: isPlaylist
            ? { name: resolved.name || null, count: resolved.songs.length, truncated: Boolean(outcome?.truncated) }
            : null,
    };
}

async function control(music, { guildId, action, value }) {
    if (!CONTROL_ACTIONS.has(action)) throw new ApiError(400, 'INVALID_ACTION', `Invalid action: ${action}`);

    const queue = music.distube.getQueue(guildId);
    if (!queue) throw new ApiError(404, 'NO_QUEUE', 'No active playback queue in this guild');

    const result = await applyControl(queue, music.translator(guildId), action, value, music);
    music.publishSync?.(guildId);
    return { success: result.ok, message: result.message };
}

function register(apiOps) {
    apiOps.register('player.state', state);
    apiOps.register('player.play', play);
    apiOps.register('player.control', control);
}

module.exports = { register, state, play, control };
