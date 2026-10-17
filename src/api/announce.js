'use strict';

const config = require('../config');
const tunnel = require('./tunnel');
const { supabase } = require('../core/supabase');
const log = require('../core/logger').createLogger('announce');

const HEARTBEAT_MS = 60_000;

function publicUrl() {
    // The auto-tunnel URL lives in the tunnel module (manager process only);
    // PUBLIC_API_URL covers shards and setups with a fixed public URL.
    const raw = String(tunnel.getTunnelUrl() || config.api.publicUrl || '').trim().replace(/\/+$/, '');
    return /^https?:\/\//i.test(raw) ? raw : '';
}

async function beat() {
    const url = publicUrl();
    if (!url || !supabase) return;
    try {
        const { error } = await supabase
            .from('bot_status')
            .upsert(
                { id: 'music', api_url: url, updated_at: new Date().toISOString() },
                { onConflict: 'id' }
            );
        if (error) log.warn(`Could not announce API URL (missing bot_status table?): ${error.message}`);
    } catch (error) {
        log.warn(`Could not announce API URL: ${error.message}`);
    }
}

/**
 * Publish this bot's public API URL to Supabase every 60s so the web
 * dashboard can discover it at runtime (no Vercel env changes needed).
 * Returns the timer, or null when PUBLIC_API_URL is not configured.
 */
function startAnnouncer() {
    if (!publicUrl()) {
        log.info('PUBLIC_API_URL not set; API discovery announcement disabled.');
        return null;
    }
    beat();
    const timer = setInterval(beat, HEARTBEAT_MS);
    timer.unref?.();
    log.ok('Announcing public API URL to Supabase every 60s.');
    return timer;
}

module.exports = { startAnnouncer, publicUrl, beat };
