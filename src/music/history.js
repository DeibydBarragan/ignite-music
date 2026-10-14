'use strict';

const path = require('node:path');
const { JsonStore } = require('../core/store');
const { supabase } = require('../core/supabase');
const logger = require('../core/logger');

const log = logger.createLogger('history');
const historyStore = new JsonStore(path.resolve(__dirname, '../../data/history'));

const MAX_LOCAL_HISTORY = 500;

/**
 * Record a song playback event in both Supabase and local store.
 */
async function recordPlay(guildId, song) {
    if (!guildId || !song) return;

    const user = (song.metadata?.requesterUser && !song.metadata.requesterUser.bot ? song.metadata.requesterUser : null)
        || (song.member?.user && !song.member.user.bot ? song.member.user : null)
        || (song.user && !song.user.bot ? song.user : null)
        || song.metadata?.requesterUser
        || song.member?.user
        || song.user;

    const record = {
        guild_id: String(guildId),
        song_id: String(song.id || song.url || ''),
        title: String(song.name || 'Desconocido'),
        artist: String(song.uploader?.name || 'Desconocido'),
        album_art: song.thumbnail || null,
        url: String(song.url || ''),
        duration: Number(song.duration) || 0,
        source: String(song.source || 'youtube'),
        user_id: user?.id ? String(user.id) : null,
        username: user ? (song.member?.displayName || user.globalName || user.username) : null,
        played_at: new Date().toISOString(),
    };

    // 1. Save to local storage cache for instant offline reliability
    try {
        const list = historyStore.get(guildId) || [];
        list.unshift(record);
        if (list.length > MAX_LOCAL_HISTORY) list.length = MAX_LOCAL_HISTORY;
        historyStore.set(guildId, list);
    } catch (e) {
        log.warn(`Failed to save local play history for guild ${guildId}: ${e.message}`);
    }

    // 2. Save to Supabase (persistent database)
    if (supabase) {
        try {
            const { error } = await supabase.from('song_plays').insert(record);
            if (error) {
                log.debug(`Supabase song_plays insert skipped/error: ${error.message}`);
            }
        } catch (e) {
            log.debug(`Supabase insert exception: ${e.message}`);
        }
    }
}

/**
 * Aggregate a list of play records into ranked songs with playCount.
 */
function aggregatePlays(records, limit = 25) {
    if (!Array.isArray(records) || records.length === 0) return [];

    const map = new Map();

    for (const item of records) {
        const key = item.url || item.song_id || item.title;
        if (!key) continue;

        let entry = map.get(key);
        if (!entry) {
            entry = {
                id: item.song_id || item.url,
                title: item.title,
                artist: item.artist,
                albumArt: item.album_art || 'https://images.unsplash.com/photo-1470225620780-dba8ba36b745?w=300&q=80',
                coverUrl: item.album_art || null,
                duration: item.duration || 0,
                source: item.source || 'youtube',
                url: item.url,
                playCount: 0,
                lastPlayed: item.played_at,
            };
            map.set(key, entry);
        }

        entry.playCount += 1;
        if (new Date(item.played_at) > new Date(entry.lastPlayed)) {
            entry.lastPlayed = item.played_at;
        }
    }

    const sorted = [...map.values()]
        .sort((a, b) => b.playCount - a.playCount || new Date(b.lastPlayed).getTime() - new Date(a.lastPlayed).getTime())
        .slice(0, limit);

    return sorted.map((song, index) => ({
        ...song,
        rank: index + 1,
    }));
}

/**
 * Get top played songs for a guild by timeframe: 'week' | 'month' | 'all'.
 */
async function getTopSongs(guildId, timeframe = 'all', limit = 25) {
    if (!guildId) return [];

    const now = Date.now();
    let minTime = null;
    if (timeframe === 'week') {
        minTime = new Date(now - 7 * 24 * 60 * 60 * 1000).toISOString();
    } else if (timeframe === 'month') {
        minTime = new Date(now - 30 * 24 * 60 * 60 * 1000).toISOString();
    }

    // Try Supabase first
    if (supabase) {
        try {
            let query = supabase
                .from('song_plays')
                .select('*')
                .eq('guild_id', String(guildId));

            if (minTime) {
                query = query.gte('played_at', minTime);
            }

            const { data, error } = await query;
            if (!error && Array.isArray(data) && data.length > 0) {
                return aggregatePlays(data, limit);
            }
        } catch (e) {
            log.debug(`Supabase getTopSongs fallback: ${e.message}`);
        }
    }

    // Fallback to local store
    try {
        const localList = historyStore.get(guildId) || [];
        const filtered = minTime
            ? localList.filter((item) => new Date(item.played_at) >= new Date(minTime))
            : localList;

        return aggregatePlays(filtered, limit);
    } catch (e) {
        log.warn(`Local getTopSongs error: ${e.message}`);
        return [];
    }
}

module.exports = {
    recordPlay,
    getTopSongs,
    aggregatePlays,
};
