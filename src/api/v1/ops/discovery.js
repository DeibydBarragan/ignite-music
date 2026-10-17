'use strict';

/**
 * Discovery operations: search, suggestions, recommendations, top songs and
 * guild introspection. Guild-scoped ones run on the owning shard; search and
 * suggestions are shard-agnostic and execute locally on the API shard.
 */

const logger = require('../../../core/logger');
const { TTLCache } = require('../../../core/cache');
const { unsupportedUrlReason } = require('../../../core/youtube-guards');
const { searchVideosDirect } = require('../../../music/innertube');
const { ApiError, serializeTrack } = require('../../contracts');

const log = logger.createLogger('api-ops');

// Same payload as the legacy /search route: 15 minutes per query+limit.
const searchVideosCache = new TTLCache({ ttlMs: 15 * 60_000, maxSize: 500 });
// Cache contextual radio recommendations per guild:song for 20 minutes.
const radioRecommendationsCache = new TTLCache({ ttlMs: 20 * 60_000, maxSize: 500 });

async function suggest(music, { q = '' }) {
    const query = String(q).trim();
    if (query.length < 2 || query.length > 100 || /^https?:\/\//i.test(query)) {
        return { query, suggestions: [] };
    }
    try {
        const { suggestions } = require('../../../music/suggest');
        return { query, suggestions: await suggestions(query) };
    } catch (error) {
        log.warn(`Suggest error: ${error.message}`);
        return { query, suggestions: [] };
    }
}

async function search(music, { q = '', limit = 6 }) {
    const query = String(q).trim();
    const capped = Math.min(Math.max(Number(limit) || 6, 1), 15);
    if (query.length < 2 || query.length > 100 || /^https?:\/\//i.test(query)) {
        return { query, results: [] };
    }
    const cacheKey = `${capped}|${query.toLowerCase()}`;
    try {
        const results = await searchVideosCache.wrap(cacheKey, async () => {
            try {
                const direct = await searchVideosDirect(query, capped);
                if (direct && direct.length > 0) return direct;
            } catch (innertubeError) {
                log.debug(`Direct Innertube search failed, falling back to yt-dlp: ${innertubeError.message}`);
            }
            const searchResults = await music.youtube.search(query, capped);
            return searchResults.map((song) => serializeTrack(song)).filter(Boolean);
        });
        return { query, results };
    } catch (error) {
        log.warn(`Search error: ${error.message}`);
        return { query, results: [] };
    }
}

async function recommendations(music, { guildId, force = false }) {
    const queue = music.distube.getQueue(guildId);
    const currentSong = queue?.songs?.[0];

    if (currentSong) {
        const songKey = String(currentSong.id || currentSong.url || '');
        const cacheKey = `${guildId}:${songKey}`;

        if (force) {
            radioRecommendationsCache.delete(cacheKey);
        }

        try {
            const cachedResult = await radioRecommendationsCache.wrap(cacheKey, async () => {
                let related = [];
                if (typeof music.youtube?.getRelatedSongs === 'function') {
                    related = await music.youtube.getRelatedSongs(currentSong);
                }
                if (!related || related.length === 0) {
                    const query = `${currentSong.uploader?.name || ''} ${currentSong.name || ''}`.trim();
                    if (query && typeof music.youtube?.search === 'function') {
                        const results = await music.youtube.search(query, 12, guildId);
                        related = results.filter((song) => song && song.id !== currentSong.id);
                    }
                }
                if (related && related.length > 0) {
                    return {
                        success: true,
                        source: 'radio',
                        basedOn: serializeTrack(currentSong),
                        songs: related.slice(0, 15).map((song) => serializeTrack(song)).filter(Boolean),
                    };
                }
                return null;
            });

            if (cachedResult && cachedResult.songs?.length > 0) {
                return cachedResult;
            }
        } catch (error) {
            log.warn(`Contextual radio error: ${error.message}`);
        }
    }

    try {
        const { getTopSongs } = require('../../../music/top-songs');
        const songs = await getTopSongs(guildId, 'all', 15);
        return { success: true, source: 'top', basedOn: null, songs };
    } catch (error) {
        log.warn(`Recommendations top fallback error: ${error.message}`);
        return { success: true, source: 'empty', basedOn: null, songs: [] };
    }
}

async function topSongs(music, { guildId, timeframe = 'week', limit = 100 }) {
    const { getTopSongs } = require('../../../music/top-songs');
    const songs = await getTopSongs(guildId, timeframe, limit);
    return { success: true, guildId, timeframe, total: songs.length, songs };
}

async function voiceStatus(music, { guildId, userId = null }) {
    const guild = music.client.guilds.cache.get(guildId);
    if (!guild) throw new ApiError(404, 'guild_not_found', 'Guild not found');

    const botChannel = guild.members.me?.voice?.channel;
    let userChannel = null;
    if (userId) {
        try {
            const member = guild.members.cache.get(userId) || await guild.members.fetch(userId).catch(() => null);
            userChannel = member?.voice?.channel || null;
        } catch {
            // ignore: user not found / not fetchable
        }
    }

    return {
        bot: botChannel ? { id: botChannel.id, name: botChannel.name } : null,
        user: userChannel ? { id: userChannel.id, name: userChannel.name } : null,
        canControl: Boolean(botChannel && userChannel && botChannel.id === userChannel.id),
    };
}

async function channels(music, { guildId }) {
    const guild = music.client.guilds.cache.get(guildId);
    if (!guild) throw new ApiError(404, 'guild_not_found', 'Guild not found');

    await guild.channels.fetch().catch(() => null);

    const botId = music.client.user?.id;
    const list = [...guild.channels.cache.filter((channel) => channel.isVoiceBased()).values()]
        .sort((a, b) => (a.position ?? 0) - (b.position ?? 0))
        .map((channel) => {
            const humans = channel.members.filter((member) => !member.user.bot);
            return {
                id: channel.id,
                name: channel.name,
                bitrate: channel.bitrate,
                userLimit: channel.userLimit || 0,
                userCount: humans.size,
                botPresent: botId ? channel.members.has(botId) : false,
                members: [...humans.values()].slice(0, 25).map((member) => ({
                    id: member.id,
                    username: member.user.username,
                    avatar: typeof member.user.displayAvatarURL === 'function' ? member.user.displayAvatarURL() : null,
                    bot: member.user.bot,
                })),
            };
        });

    return { guildId, total: list.length, channels: list };
}

async function resolvePlaylist(music, { guildId, url }) {
    const guild = music.client.guilds.cache.get(guildId);
    if (!guild) throw new ApiError(404, 'guild_not_found', 'Guild not found');

    try {
        const playlists = require('../../../music/playlists');
        const result = await playlists.resolvePlaylist(music, guild, url, { member: guild.members.me });
        return {
            success: true,
            kind: result.kind,
            name: result.name,
            source: result.source,
            count: result.tracks.length,
            truncated: result.truncated,
            tracks: result.tracks,
        };
    } catch (error) {
        const unsupported = unsupportedUrlReason(url);
        if (unsupported) throw new ApiError(422, 'UNSUPPORTED_URL', unsupported);
        throw new ApiError(422, 'RESOLVE_FAILED', error.message);
    }
}

function register(apiOps) {
    apiOps.register('discovery.suggest', suggest);
    apiOps.register('discovery.search', search);
    apiOps.register('discovery.recommendations', recommendations);
    apiOps.register('discovery.topSongs', topSongs);
    apiOps.register('guild.voiceStatus', voiceStatus);
    apiOps.register('guild.channels', channels);
    apiOps.register('playlist.resolve', resolvePlaylist);
}

module.exports = { register, suggest, search, recommendations, topSongs, voiceStatus, channels, resolvePlaylist };
