'use strict';

const { Playlist, DisTubeError } = require('distube');
const config = require('../config');
const { isLyricSong } = require('./plugins');
const { supabase } = require('../core/supabase');
const log = require('../core/logger').createLogger('playlists');

const TABLE = 'user_playlists';
const FALLBACK_COVER = 'https://images.unsplash.com/photo-1470225620780-dba8ba36b745?w=300&q=80';

function isSoundCloudUrl(url) {
    return String(url || '').toLowerCase().includes('soundcloud.com');
}

function detectSource(url) {
    const lower = String(url || '').toLowerCase();
    if (lower.includes('spotify.com')) return 'spotify';
    if (lower.includes('youtube.com') || lower.includes('youtu.be')) return 'youtube';
    return 'spotify';
}

function deriveName(url, customName) {
    const wanted = String(customName || '').trim().slice(0, 60);
    if (wanted) return wanted;
    try {
        const parsed = new URL(String(url).trim());
        const parts = parsed.pathname.split('/').filter(Boolean);
        const last = parts[parts.length - 1] || 'Playlist';
        return decodeURIComponent(last).replace(/[-_]+/g, ' ').slice(0, 60) || 'Nueva playlist';
    } catch {
        return 'Nueva playlist';
    }
}

/**
 * DisTube Song -> frontend Song contract (same shape as api/server.js formatSong).
 */
function toFrontendTrack(song) {
    if (!song) return null;
    return {
        id: song.id || song.url,
        title: song.name || 'Unknown Title',
        artist: song.uploader?.name || 'Unknown Artist',
        albumArt: song.thumbnail || FALLBACK_COVER,
        coverUrl: song.thumbnail || null,
        duration: song.duration || 0,
        formattedDuration: song.formattedDuration || '00:00',
        source: song.source || 'youtube',
        url: song.url,
        category: 'hits',
        isLive: Boolean(song.isLive),
        isLyric: isLyricSong(song.url),
    };
}

/**
 * Resolve a playlist/track URL with DisTube without joining voice.
 * Returns { kind: 'playlist'|'track', name, tracks, truncated, source }.
 */
async function resolvePlaylist(manager, guild, url, { member = null, maxSize = null } = {}) {
    const clean = String(url || '').trim();
    if (!clean) throw new Error('EMPTY_URL');
    if (isSoundCloudUrl(clean)) {
        throw new DisTubeError(
            'NOT_SUPPORTED_URL',
            'SoundCloud is no longer supported. Use a YouTube or Spotify link.'
        );
    }
    const limit = Math.max(1, maxSize || config.bot.maxPlaylistSize || 200);
    const resolveAs = member || guild?.members?.me || null;
    let resolved;
    try {
        resolved = await manager.distube.handler.resolve(clean, {
            member: resolveAs,
            metadata: guild ? { guildId: guild.id } : undefined,
        });
    } catch (error) {
        throw error;
    }

    if (resolved instanceof Playlist) {
        const songs = Array.isArray(resolved.songs) ? resolved.songs : [];
        const truncated = songs.length > limit;
        const tracks = songs.slice(0, limit).map(toFrontendTrack).filter(Boolean);
        return {
            kind: 'playlist',
            name: resolved.name || deriveName(clean),
            tracks,
            truncated,
            source: detectSource(clean),
        };
    }
    const track = toFrontendTrack(resolved);
    return {
        kind: 'track',
        name: resolved?.name || deriveName(clean),
        tracks: track ? [track] : [],
        truncated: false,
        source: detectSource(clean),
    };
}

function mapRow(row) {
    const tracks = Array.isArray(row.tracks) ? row.tracks : [];
    return {
        id: row.id,
        user_id: row.user_id,
        name: row.name || 'Playlist sin nombre',
        source: row.source || 'spotify',
        url: row.url,
        cover: row.cover || tracks[0]?.albumArt || FALLBACK_COVER,
        tracks,
        synced_at: row.synced_at || null,
        created_at: row.created_at,
    };
}

async function fetchUserPlaylists(userId, { limit = 100 } = {}) {
    if (!supabase) throw new Error('DB_UNAVAILABLE');
    const { data, error } = await supabase
        .from(TABLE)
        .select('*')
        .eq('user_id', String(userId))
        .order('created_at', { ascending: false })
        .limit(limit);
    if (error) throw error;
    return (data || []).map(mapRow);
}

/**
 * Insert or update a playlist for a user (matched by user_id + url).
 * `tracks` is only a snapshot (caché de lectura); the URL is the source of truth.
 */
async function upsertUserPlaylist({ userId, name, url, source, cover, tracks, synced_at = null }) {
    if (!supabase) throw new Error('DB_UNAVAILABLE');
    const uid = String(userId);
    const cleanUrl = String(url).trim();
    const payloadTracks = Array.isArray(tracks) ? tracks : [];
    const payload = {
        user_id: uid,
        name: String(name || deriveName(cleanUrl)).slice(0, 60),
        source: source || detectSource(cleanUrl),
        url: cleanUrl,
        cover: cover || payloadTracks[0]?.albumArt || FALLBACK_COVER,
        tracks: payloadTracks,
        synced_at,
    };

    const { data: existing } = await supabase
        .from(TABLE)
        .select('id')
        .eq('user_id', uid)
        .eq('url', cleanUrl)
        .maybeSingle();

    if (existing?.id) {
        const { data, error } = await supabase
            .from(TABLE)
            .update(payload)
            .eq('id', existing.id)
            .select()
            .single();
        if (error) throw error;
        return { playlist: mapRow(data), created: false };
    }

    const { data, error } = await supabase
        .from(TABLE)
        .insert(payload)
        .select()
        .single();
    if (error) throw error;
    return { playlist: mapRow(data), created: true };
}

module.exports = {
    TABLE,
    FALLBACK_COVER,
    detectSource,
    isSoundCloudUrl,
    deriveName,
    toFrontendTrack,
    resolvePlaylist,
    fetchUserPlaylists,
    upsertUserPlaylist,
    mapRow,
    log,
};
