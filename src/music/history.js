'use strict';

const fs = require('node:fs');
const fsp = require('node:fs/promises');
const path = require('node:path');
const config = require('../config');
const { JsonStore } = require('../core/store');
const { supabase } = require('../core/supabase');
const { resolveRequester, requesterName } = require('../core/user');
const logger = require('../core/logger');

const log = logger.createLogger('history');
const historyStore = new JsonStore(path.join(config.dataDir, 'history'));

const MAX_LOCAL_HISTORY = 500;
const EXCLUSIONS_FILE = path.join(config.dataDir, 'history', 'exclusions.json');

// Dates ("YYYY-MM-DD") excluded from top-song aggregation, cached in memory.
// Loaded asynchronously once and refreshed by a directory watcher; the
// aggregation hot path never touches the filesystem.
let excludedDates = new Set();
let exclusionsWatcher = null;

/**
 * Load excluded dates into memory. Never throws: a missing file clears the
 * list, any other error is logged and keeps the last known list.
 */
async function loadExclusions() {
    try {
        const raw = await fsp.readFile(EXCLUSIONS_FILE, 'utf8');
        const data = JSON.parse(raw);
        excludedDates = new Set(Array.isArray(data?.dates) ? data.dates.map(String) : []);
    } catch (error) {
        if (error.code === 'ENOENT') {
            excludedDates = new Set();
        } else {
            log.warn(`Could not load history exclusions: ${error.message}`);
        }
    }
    ensureExclusionsWatcher();
    return excludedDates;
}

function ensureExclusionsWatcher() {
    if (exclusionsWatcher) return;
    try {
        exclusionsWatcher = fs.watch(path.dirname(EXCLUSIONS_FILE), (_event, filename) => {
            if (!filename || filename === path.basename(EXCLUSIONS_FILE)) {
                loadExclusions().catch(() => undefined);
            }
        });
        if (exclusionsWatcher.unref) exclusionsWatcher.unref();
    } catch {
        // Data directory does not exist yet; retried on the next load.
    }
}

// Warm the caches without blocking startup; the watcher keeps exclusions fresh.
loadExclusions().catch(() => undefined);
historyStore.initialize().catch((error) => log.warn(`Could not load local history: ${error.message}`));

function isExcludedPlay(item) {
    if (!item?.played_at) return false;
    return excludedDates.has(String(item.played_at).slice(0, 10));
}

/** Snapshot of the excluded dates ("YYYY-MM-DD"), for the top_songs RPC. */
function getExcludedDates() {
    return [...excludedDates];
}

/**
 * Record a song playback event in both Supabase and local store.
 */
async function recordPlay(guildId, song) {
    if (!guildId || !song) return;

    const user = resolveRequester(song);

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
        username: user ? requesterName(song, user) : null,
        played_at: new Date().toISOString(),
    };

    // 1. Save to local storage cache for instant offline reliability
    try {
        const list = historyStore.get(guildId) || [];
        list.unshift(record);
        if (list.length > MAX_LOCAL_HISTORY) list.length = MAX_LOCAL_HISTORY;
        await historyStore.set(guildId, list);
        ensureExclusionsWatcher();
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

module.exports = {
    recordPlay,
    loadExclusions,
    isExcludedPlay,
    getExcludedDates,
    historyStore,
};
