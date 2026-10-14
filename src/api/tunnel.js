'use strict';

const { spawn } = require('node:child_process');
const fs = require('node:fs');
const log = require('../core/logger').createLogger('tunnel');

let tunnelProcess = null;
let tunnelUrl = null;

const COMMON_WINDOWS_PATHS = [
    'cloudflared',
    'C:\\Program Files (x86)\\cloudflared\\cloudflared.exe',
    'C:\\Program Files\\cloudflared\\cloudflared.exe',
];

function resolveBinary() {
    if (process.platform === 'win32') {
        for (const candidate of COMMON_WINDOWS_PATHS) {
            if (candidate === 'cloudflared') continue;
            if (fs.existsSync(candidate)) return candidate;
        }
    }
    return 'cloudflared';
}

/**
 * Start a Cloudflare Quick Tunnel to expose the local REST API.
 * Captures the generated trycloudflare.com URL and returns it.
 *
 * @param {number} port Local port to expose (e.g. 3001)
 * @param {number} timeoutMs Max wait time for tunnel generation (default 25s)
 * @returns {Promise<string|null>} The public https://*.trycloudflare.com URL or null on failure.
 */
function startTunnel(port = 3001, timeoutMs = 25_000) {
    if (tunnelUrl && tunnelProcess && !tunnelProcess.killed) {
        return Promise.resolve(tunnelUrl);
    }

    const binary = resolveBinary();

    return new Promise((resolve) => {
        let resolved = false;
        const timer = setTimeout(() => {
            if (!resolved) {
                resolved = true;
                log.warn('Timeout waiting for Cloudflare tunnel URL.');
                resolve(null);
            }
        }, timeoutMs);
        timer.unref?.();

        try {
            log.info(`Starting Cloudflare tunnel for http://localhost:${port}...`);
            const child = spawn(binary, ['tunnel', '--url', `http://localhost:${port}`], {
                stdio: ['ignore', 'pipe', 'pipe'],
                windowsHide: true,
            });

            tunnelProcess = child;

            const onData = (chunk) => {
                const text = chunk.toString();
                // Matches https://[words].trycloudflare.com
                const match = text.match(/https:\/\/[a-zA-Z0-9-]+\.trycloudflare\.com/);
                if (match && !resolved) {
                    resolved = true;
                    clearTimeout(timer);
                    tunnelUrl = match[0];
                    log.ok(`Cloudflare tunnel active: ${tunnelUrl}`);
                    resolve(tunnelUrl);
                }
            };

            child.stdout.on('data', onData);
            child.stderr.on('data', onData);

            child.on('error', (err) => {
                log.warn(`Could not start cloudflared (${err.message}). Is it installed?`);
                if (!resolved) {
                    resolved = true;
                    clearTimeout(timer);
                    resolve(null);
                }
            });

            child.on('exit', (code) => {
                if (!resolved) {
                    resolved = true;
                    clearTimeout(timer);
                    log.warn(`cloudflared exited early with code ${code}`);
                    resolve(null);
                } else {
                    log.info(`Cloudflare tunnel stopped (code ${code ?? 'signal'}).`);
                }
                tunnelProcess = null;
                tunnelUrl = null;
            });
        } catch (err) {
            log.warn(`Failed to spawn cloudflared: ${err.message}`);
            if (!resolved) {
                resolved = true;
                clearTimeout(timer);
                resolve(null);
            }
        }
    });
}

/**
 * Terminate the running tunnel process if any.
 */
function stopTunnel() {
    if (tunnelProcess && !tunnelProcess.killed) {
        log.info('Stopping Cloudflare tunnel...');
        try {
            tunnelProcess.kill('SIGTERM');
        } catch {
            try {
                tunnelProcess.kill();
            } catch {
                // ignore
            }
        }
        tunnelProcess = null;
        tunnelUrl = null;
    }
}

function getTunnelUrl() {
    return tunnelUrl;
}

module.exports = {
    startTunnel,
    stopTunnel,
    getTunnelUrl,
};
