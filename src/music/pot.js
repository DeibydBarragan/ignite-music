'use strict';

const fs = require('node:fs');
const { spawn } = require('node:child_process');
const config = require('../config');
const log = require('../core/logger').createLogger('pot');

/**
 * Automatic YouTube PO (Proof of Origin) token renewals.
 *
 * Datacenter IPs get flagged by YouTube ("Sign in to confirm you're not a bot")
 * and exported browser cookies die fast there. A fresh PO token minted locally
 * (via bgutil-ytdlp-pot-provider's generate_once.js) re-authenticates the
 * embedded player without depending on anyone's browser session.
 *
 * - Disabled when no generator is configured/found (previous behavior).
 * - Token cached until ~60 min before expiresAt, refreshed every 30 min.
 * - Never throws: on failure yt-dlp simply runs without po_token (status quo).
 */

const REFRESH_INTERVAL_MS = 30 * 60_000;
const EXPIRY_MARGIN_MS = 60 * 60_000;
const MINT_TIMEOUT_MS = 180_000;

let cache = null; // { token: string, expiresAtMs: number }
let started = false;
let inflight = null;

function generatorPath() {
    // POT_GENERATOR_PATH is the documented variable; POT_GENERATOR is kept as
    // a legacy alias. No machine-specific paths are hardcoded anymore.
    const configured = config.ytdlp.potGeneratorPath || config.ytdlp.potGenerator || '';
    if (configured && fs.existsSync(configured)) return configured;
    return null;
}

function mint() {
    const generator = generatorPath();
    if (!generator) return Promise.resolve(null);

    return new Promise((resolve) => {
        let stdout = '';
        let settled = false;
        const finish = (value) => {
            if (settled) return;
            settled = true;
            resolve(value);
        };

        let child;
        try {
            child = spawn(process.execPath, [generator], {
                windowsHide: true,
                stdio: ['ignore', 'pipe', 'pipe'],
                timeout: MINT_TIMEOUT_MS,
            });
        } catch (err) {
            log.warn(`PO token mint failed to spawn: ${err.message}`);
            return finish(null);
        }

        const timer = setTimeout(() => {
            try { child.kill('SIGKILL'); } catch {}
            log.warn('PO token mint timed out');
            finish(null);
        }, MINT_TIMEOUT_MS);
        if (timer.unref) timer.unref();

        child.stdout.on('data', (chunk) => {
            stdout += chunk.toString('utf8');
            if (stdout.length > 1_000_000) {
                try { child.kill('SIGKILL'); } catch {}
            }
        });
        child.on('error', (err) => {
            clearTimeout(timer);
            log.warn(`PO token mint error: ${err.message}`);
            finish(null);
        });
        child.on('close', () => {
            clearTimeout(timer);
            try {
                const start = stdout.indexOf('{');
                const parsed = JSON.parse(start >= 0 ? stdout.slice(start) : stdout);
                if (parsed && typeof parsed.poToken === 'string' && parsed.poToken.length > 10) {
                    return finish({
                        token: parsed.poToken,
                        expiresAtMs: parsed.expiresAt ? Date.parse(parsed.expiresAt) : Date.now() + 6 * 3600_000,
                    });
                }
                log.warn('PO token mint returned no usable token');
            } catch (err) {
                log.warn(`PO token mint parse failed: ${err.message}`);
            }
            finish(null);
        });
    });
}

async function refresh() {
    if (inflight) return inflight;
    inflight = mint()
        .then((result) => {
            if (result && Number.isFinite(result.expiresAtMs)) {
                cache = result;
                log.info(`PO token renewed, valid until ${new Date(result.expiresAtMs).toISOString()}`);
            }
            return cache;
        })
        .catch((err) => {
            log.warn(`PO token refresh failed: ${err?.message || err}`);
            return cache;
        })
        .finally(() => {
            inflight = null;
        });
    return inflight;
}

function ensureStarted() {
    if (started) return;
    started = true;
    if (!generatorPath()) {
        log.info('PO token auto-renew disabled (no generator found)');
        return;
    }
    refresh();
    const timer = setInterval(() => {
        refresh();
    }, REFRESH_INTERVAL_MS);
    if (timer.unref) timer.unref();
}

/**
 * Current usable PO token, or null when unavailable.
 * Triggers a background refresh when missing/expiring; never blocks or throws.
 */
function currentPoToken() {
    try {
        ensureStarted();
    } catch {
        return cache?.token || null;
    }
    if (cache && Date.now() < cache.expiresAtMs - EXPIRY_MARGIN_MS) {
        return cache.token;
    }
    refresh();
    return cache?.token || null;
}

/**
 * Like currentPoToken(), but waits for a pending mint on a cold start so the
 * first yt-dlp calls still run with a token. Never throws.
 */
async function currentPoTokenAsync() {
    if (cache && Date.now() < cache.expiresAtMs - EXPIRY_MARGIN_MS) {
        ensureStarted();
        return cache.token;
    }
    ensureStarted();
    if (!generatorPath()) return cache?.token || null;
    const result = await refresh();
    return result?.token || null;
}

/**
 * Start the renewal timer before the first download needs it (preflight / shard boot).
 */
function startPoTokenRenewal() {
    ensureStarted();
}

module.exports = { currentPoToken, currentPoTokenAsync, startPoTokenRenewal, refreshPoToken: refresh };
