'use strict';

/**
 * Queue operations. Every mutation of the DisTube songs array happens under
 * the guild's play lock (manager.withPlayLock) so concurrent API calls can
 * never interleave splices. Undo stores only serialized metadata: streams are
 * re-resolved by the plugins when the songs play again, so expired audio URLs
 * are never reinserted.
 */

const { ApiError, serializeTrack, ensureTrackId } = require('../../contracts');
const actions = require('../../../music/actions');

const UNDO_TTL_MS = 60_000;
const SWEEP_INTERVAL_MS = 30_000;

const undoQueueCache = new Map();
let sweepTimer = null;

function ensureSweep() {
    if (sweepTimer) return;
    sweepTimer = setInterval(() => {
        const now = Date.now();
        for (const [guildId, entry] of undoQueueCache) {
            if (now > entry.expiresAt) undoQueueCache.delete(guildId);
        }
    }, SWEEP_INTERVAL_MS);
    sweepTimer.unref();
}

function queueOf(music, guildId) {
    const queue = music.distube.getQueue(guildId);
    if (!queue) throw new ApiError(404, 'NO_QUEUE', 'No queue found');
    return queue;
}

async function removeIndex(music, { guildId, index }) {
    return music.withPlayLock(guildId, async () => {
        const queue = queueOf(music, guildId);
        const idx = Number(index);
        if (!Number.isInteger(idx) || idx < 1 || idx >= queue.songs.length) {
            throw new ApiError(400, 'INVALID_INDEX', 'Invalid song queue index');
        }
        const [removed] = queue.songs.splice(idx, 1);
        music.changed(queue);
        return { success: true, removed: removed ? serializeTrack(removed) : null };
    });
}

async function removeTrack(music, { guildId, trackId }) {
    return music.withPlayLock(guildId, async () => {
        const queue = queueOf(music, guildId);
        const idx = queue.songs.findIndex((song, i) => i > 0 && ensureTrackId(song) === trackId);
        if (idx < 1) throw new ApiError(404, 'TRACK_NOT_FOUND', 'No upcoming track with that trackId in this queue');
        const [removed] = queue.songs.splice(idx, 1);
        music.changed(queue);
        return { success: true, removed: removed ? serializeTrack(removed) : null };
    });
}

/** Shared by move (from index) and order (resolved trackId). */
async function moveInQueue(music, guildId, queue, from, to) {
    const outcome = actions.move(queue, music.translator(guildId), from, to);
    if (!outcome.ok) throw new ApiError(400, 'INVALID_MOVE', outcome.message);
    music.changed(queue);
    return { success: true, message: outcome.message };
}

async function move(music, { guildId, from, to }) {
    return music.withPlayLock(guildId, () => {
        const queue = queueOf(music, guildId);
        return moveInQueue(music, guildId, queue, Number(from), Number(to));
    });
}

async function order(music, { guildId, trackId, to }) {
    return music.withPlayLock(guildId, async () => {
        const queue = queueOf(music, guildId);
        const from = queue.songs.findIndex((song, i) => i > 0 && ensureTrackId(song) === trackId);
        if (from < 1) throw new ApiError(404, 'TRACK_NOT_FOUND', 'No upcoming track with that trackId in this queue');
        return moveInQueue(music, guildId, queue, from, Number(to));
    });
}

async function jump(music, { guildId, position }) {
    return music.withPlayLock(guildId, async () => {
        const queue = queueOf(music, guildId);
        const outcome = await actions.jump(queue, music.translator(guildId), Number(position));
        if (!outcome.ok) throw new ApiError(400, 'INVALID_JUMP', outcome.message);
        return { success: true, message: outcome.message };
    });
}

async function clear(music, { guildId }) {
    return music.withPlayLock(guildId, async () => {
        const queue = queueOf(music, guildId);
        const count = queue.songs.length - 1;
        if (queue.songs.length > 1) {
            const removed = queue.songs.splice(1);
            ensureSweep();
            undoQueueCache.set(guildId, {
                songs: removed.map((song) => music.sessions.serializeSong(song)),
                expiresAt: Date.now() + UNDO_TTL_MS,
            });
            music.changed(queue);
        }
        return { success: true, cleared: count };
    });
}

async function undo(music, { guildId }) {
    return music.withPlayLock(guildId, async () => {
        const cached = undoQueueCache.get(guildId);
        if (!cached || !Array.isArray(cached.songs) || cached.songs.length === 0) {
            throw new ApiError(400, 'UNDO_EMPTY', 'No hay canciones recientes para restaurar.');
        }
        if (Date.now() > cached.expiresAt) {
            undoQueueCache.delete(guildId);
            throw new ApiError(400, 'UNDO_EXPIRED', 'El tiempo para deshacer ha expirado.');
        }

        const queue = music.distube.getQueue(guildId);
        if (!queue) throw new ApiError(404, 'NO_ACTIVE_PLAYER', 'El reproductor no está activo en este servidor.');
        const guild = music.client.guilds.cache.get(guildId);
        if (!guild) throw new ApiError(404, 'guild_not_found', 'Guild not found');

        let songs = [];
        try {
            songs = music.sessions.buildSongs({ songs: cached.songs }, guild);
        } catch {
            songs = [];
        }
        if (songs.length === 0) throw new ApiError(422, 'UNDO_FAILED', 'No se pudieron restaurar las canciones.');

        queue.songs.push(...songs);
        undoQueueCache.delete(guildId);
        music.changed(queue);
        return { success: true, restored: songs.length, message: `${songs.length} canciones restauradas a la cola.` };
    });
}

function register(apiOps) {
    apiOps.register('queue.removeIndex', removeIndex);
    apiOps.register('queue.removeTrack', removeTrack);
    apiOps.register('queue.move', move);
    apiOps.register('queue.order', order);
    apiOps.register('queue.jump', jump);
    apiOps.register('queue.clear', clear);
    apiOps.register('queue.undo', undo);
}

module.exports = { register, removeIndex, removeTrack, move, order, jump, clear, undo };
