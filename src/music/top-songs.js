'use strict';

/**
 * Top played songs per guild. Aggregation runs in Postgres via the top_songs
 * RPC (see supabase/migrations/20261016_top_songs.sql) so thousands of
 * song_plays rows are never pulled into memory; older deployments without the
 * RPC fall back to a bounded scan (never the whole table), and a local
 * JsonStore fallback keeps the dashboard working offline.
 */

const { supabase } = require('../core/supabase');
const { FALLBACK_COVER } = require('../core/constants');
const logger = require('../core/logger');
const { isLyricSong } = require('./plugins');
const { historyStore, isExcludedPlay, getExcludedDates } = require('./history');

const log = logger.createLogger('top-songs');

const TOP_SONGS_LIMIT = 50;
const SUPABASE_SCAN_LIMIT = 2000;

/**
 * Complete a top-songs entry into the frontend Song contract: the dashboard
 * schema rejects tracks without category/formattedDuration.
 */
function completeSong(entry, rank) {
    const duration = Number(entry.duration) || 0;
    const url = entry.url || '';
    return {
        id: entry.id || url,
        title: entry.title || 'Desconocido',
        artist: entry.artist || 'Desconocido',
        albumArt: entry.albumArt || entry.album_art || FALLBACK_COVER,
        coverUrl: entry.coverUrl ?? entry.album_art ?? null,
        duration,
        formattedDuration: `${Math.floor(duration / 60)}:${String(Math.floor(duration % 60)).padStart(2, '0')}`,
        // The dashboard only understands youtube/spotify.
        source: entry.source === 'spotify' ? 'spotify' : 'youtube',
        url,
        category: 'hits',
        isLyric: isLyricSong(url),
        isLive: false,
        isAutoplay: false,
        playCount: Number(entry.playCount) || 0,
        rank,
        lastPlayed: entry.lastPlayed || null,
    };
}

/**
 * Aggregate raw play records (bounded fallback path) into ranked songs.
 */
function aggregatePlays(records, limit = TOP_SONGS_LIMIT) {
    if (!Array.isArray(records) || records.length === 0) return [];

    const map = new Map();
    for (const item of records) {
        if (isExcludedPlay(item)) continue;
        const key = item.url || item.song_id || item.title;
        if (!key) continue;

        let entry = map.get(key);
        if (!entry) {
            entry = {
                id: item.song_id || item.url,
                title: item.title,
                artist: item.artist,
                album_art: item.album_art,
                duration: item.duration,
                source: item.source,
                url: item.url,
                playCount: 0,
                lastPlayed: item.played_at,
            };
            map.set(key, entry);
        }
        entry.playCount += 1;
        if (new Date(item.played_at) > new Date(entry.lastPlayed)) entry.lastPlayed = item.played_at;
    }

    return [...map.values()]
        .sort((a, b) => b.playCount - a.playCount || new Date(b.lastPlayed).getTime() - new Date(a.lastPlayed).getTime())
        .slice(0, limit)
        .map((entry, index) => completeSong(entry, index + 1));
}

/**
 * Top played songs for a guild by timeframe: 'week' | 'month' | 'all'.
 */
async function getTopSongs(guildId, timeframe = 'all', limit = TOP_SONGS_LIMIT) {
    if (!guildId) return [];
    const capped = Math.min(Math.max(Number(limit) || TOP_SONGS_LIMIT, 1), TOP_SONGS_LIMIT);

    const now = Date.now();
    let minTime = null;
    if (timeframe === 'week') minTime = new Date(now - 7 * 24 * 60 * 60 * 1000).toISOString();
    else if (timeframe === 'month') minTime = new Date(now - 30 * 24 * 60 * 60 * 1000).toISOString();

    if (supabase) {
        // 1. Database-side aggregation (bounded result, exclusions included).
        try {
            const { data, error } = await supabase.rpc('top_songs', {
                p_guild_id: String(guildId),
                p_since: minTime,
                p_limit: capped,
                p_excluded_dates: getExcludedDates(),
            });
            if (error) {
                log.debug(`top_songs RPC unavailable (${error.message}); using bounded scan`);
            } else if (Array.isArray(data)) {
                return data.map((row, index) => completeSong({
                    id: row.song_id,
                    title: row.title,
                    artist: row.artist,
                    album_art: row.album_art,
                    duration: row.duration,
                    source: row.source,
                    url: row.url,
                    playCount: row.play_count,
                    lastPlayed: row.last_played,
                }, index + 1));
            }
        } catch (error) {
            log.debug(`top_songs RPC failed: ${error.message}`);
        }

        // 2. Bounded scan for deployments without the RPC.
        try {
            let query = supabase
                .from('song_plays')
                .select('*')
                .eq('guild_id', String(guildId))
                .order('played_at', { ascending: false })
                .limit(SUPABASE_SCAN_LIMIT);
            if (minTime) query = query.gte('played_at', minTime);

            const { data, error } = await query;
            if (!error && Array.isArray(data) && data.length > 0) {
                return aggregatePlays(data, capped);
            }
        } catch (error) {
            log.debug(`Supabase getTopSongs fallback: ${error.message}`);
        }
    }

    // 3. Local store fallback.
    try {
        const localList = historyStore.get(guildId) || [];
        const filtered = minTime
            ? localList.filter((item) => new Date(item.played_at) >= new Date(minTime))
            : localList;
        return aggregatePlays(filtered, capped);
    } catch (error) {
        log.warn(`Local getTopSongs error: ${error.message}`);
        return [];
    }
}

module.exports = { getTopSongs, aggregatePlays, TOP_SONGS_LIMIT, completeSong };
