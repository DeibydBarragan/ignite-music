'use strict';

const { createClient } = require('@supabase/supabase-js');
const config = require('../config');
const logger = require('./logger');

const log = logger.createLogger('supabase');

// Credentials come exclusively from the environment (never committed).
// The service role key is preferred: with the strict RLS from
// 20261016_lockdown_rls.sql the anon key can no longer write play history,
// favorites or playlists.
const SUPABASE_URL = String(config.supabase.url || '').trim();
const SUPABASE_KEY = String(config.supabase.serviceRoleKey || config.supabase.key || '').trim();

let supabase = null;
let initError = null;

if (SUPABASE_URL && SUPABASE_KEY) {
    try {
        supabase = createClient(SUPABASE_URL, SUPABASE_KEY, {
            auth: { persistSession: false, autoRefreshToken: false },
        });
    } catch (err) {
        initError = err;
        log.error('Failed to create Supabase client:', err.message);
    }
} else if (SUPABASE_URL || SUPABASE_KEY) {
    log.warn('Supabase is partially configured (SUPABASE_URL / SUPABASE_KEY): disabled until both are set');
}

/**
 * Verify URL + key against the project before anything logs a success.
 * Never throws: returns { ok: true } or { ok: false, reason }.
 */
async function verifyConnection(timeoutMs = 8000) {
    if (!supabase) {
        return {
            ok: false,
            reason: initError
                ? `client init failed: ${initError.message}`
                : 'SUPABASE_URL and SUPABASE_KEY are not set',
        };
    }

    try {
        const timeout = new Promise((resolve) => {
            const timer = setTimeout(() => {
                resolve({ error: { message: `timed out after ${timeoutMs} ms` } });
            }, timeoutMs);
            if (timer.unref) timer.unref();
        });

        const { error } = await Promise.race([
            supabase.from('song_plays').select('id').limit(1),
            timeout,
        ]);

        if (error) return { ok: false, reason: error.message };

        log.ok('Supabase connection verified');
        return { ok: true };
    } catch (err) {
        return { ok: false, reason: err.message };
    }
}

module.exports = { supabase, verifyConnection };
