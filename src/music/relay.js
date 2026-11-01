'use strict';

const http = require('node:http');
const fs = require('node:fs');
const fsp = require('node:fs/promises');
const path = require('node:path');
const crypto = require('node:crypto');
const { spawn } = require('node:child_process');
const config = require('../config');
const ytdlp = require('./ytdlp');
const log = require('../core/logger').createLogger('relay');

const ENTRY_TTL_MS = 6 * 3600_000;
const MAX_ENTRIES = 300;
// Large parts of yt-dlp's info that are not needed to download the chosen format.
const UNUSED_INFO_KEYS = ['automatic_captions', 'subtitles', 'thumbnails', 'heatmap', 'chapters', 'description', 'requested_subtitles'];

/** Keep only what yt-dlp needs to download the chosen format (keeps files small). */
function trimInfo(info) {
    const trimmed = { ...info };
    for (const key of UNUSED_INFO_KEYS) delete trimmed[key];
    if (Array.isArray(info.formats) && info.format_id) {
        const chosen = info.formats.filter((format) => format.format_id === info.format_id);
        if (chosen.length) trimmed.formats = chosen;
    }
    return trimmed;
}

function parseRange(header, total = null) {
    if (!header || typeof header !== 'string' || !header.startsWith('bytes=')) return null;
    const eq = header.indexOf('=');
    const parts = header.slice(eq + 1).split('-');
    if (parts.length !== 2) return null;

    const s = parts[0].trim();
    const e = parts[1].trim();

    let start = s ? parseInt(s, 10) : null;
    let end = e ? parseInt(e, 10) : null;

    if (start === null && end !== null) {
        if (total === null) return null;
        start = Math.max(0, total - end);
        end = total - 1;
    } else if (start !== null) {
        if (end === null && total !== null) {
            end = total - 1;
        }
    } else {
        return null;
    }

    if (isNaN(start) || start < 0) return null;
    if (end !== null && (isNaN(end) || end < start)) return null;
    return { start, end };
}

/** Each process (shard) has its own folder, so shards never delete each other's files. */
const processDir = () => path.join(config.dataDir, 'relay', String(process.pid));

/**
 * Local audio relay: ffmpeg reads from http://127.0.0.1/<id> while yt-dlp streams audio.
 * Handles chunked range requests, headers, PO tokens, proxies and cookies without re-extracting info.
 */
class StreamRelay {
    constructor({ dir = processDir(), spawnImpl = spawn, binary = ytdlp.binary } = {}) {
        this.dir = dir;
        this.spawn = spawnImpl;
        this.binary = binary;
        this.entries = new Map();
        this.errors = new Map();
        this.server = null;
        this.port = null;
        this.starting = null;
        fs.rmSync(dir, { recursive: true, force: true });
        fs.mkdirSync(dir, { recursive: true });
    }

    start() {
        this.starting ??= new Promise((resolve, reject) => {
            this.server = http.createServer((req, res) => this.#handle(req, res));
            this.server.on('error', reject);
            this.server.listen(0, '127.0.0.1', () => {
                this.port = /** @type {import('node:net').AddressInfo} */ (this.server.address()).port;
                this.server.unref();
                resolve(undefined);
            });
        });
        return this.starting;
    }

    /**
     * Register a stream (`info`: yt-dlp info dict, `url`: media URL). Returns local URL for ffmpeg.
     */
    async register({ info = null, url = null, proxy = null }) {
        await this.start();
        const id = crypto.randomBytes(12).toString('hex');
        const entry = { proxy, url, infoFile: null, format: null, timer: null, pending: null, activeDownload: null };
        if (info) {
            entry.infoFile = path.join(this.dir, `${id}.json`);
            entry.format = info.format_id || 'bestaudio/best';
            await fsp.mkdir(this.dir, { recursive: true });
            await fsp.writeFile(entry.infoFile, JSON.stringify(trimInfo(info)));
        }
        entry.timer = setTimeout(() => this.#forget(id), ENTRY_TTL_MS);
        entry.timer.unref();
        this.entries.set(id, entry);
        // Map keeps insertion order: drop the oldest streams beyond the limit.
        while (this.entries.size > MAX_ENTRIES) this.#forget(this.entries.keys().next().value);
        return `http://127.0.0.1:${this.port}/${id}`;
    }

    /**
     * Why the last download for this relay URL failed (yt-dlp's message), if it did.
     */
    errorFor(relayUrl) {
        const id = this.#idOf(relayUrl);
        return id ? this.errors.get(id) || null : null;
    }

    isRelayUrl(url) {
        return Boolean(this.#idOf(url));
    }

    #idOf(url) {
        if (!url || !this.port) return null;
        const prefix = `http://127.0.0.1:${this.port}/`;
        return typeof url === 'string' && url.startsWith(prefix) ? url.slice(prefix.length) : null;
    }

    args(entry) {
        const source = entry.infoFile
            ? ['--load-info-json', entry.infoFile, '-f', entry.format]
            : ['--', entry.url];
        return [
            ...ytdlp.baseArgs({ proxy: entry.proxy }),
            '--quiet', '--no-part', '--no-playlist',
            '-o', '-',
            ...source,
        ];
    }

    /**
     * Start download early to fail-fast on 403, bot checks or deleted videos before ffmpeg starts.
     */
    warm(relayUrl, { timeoutMs = 25_000, holdMs = 60_000 } = {}) {
        const id = this.#idOf(relayUrl);
        const entry = id && this.entries.get(id);
        if (!entry) return Promise.reject(new Error('Unknown relay stream'));
        this.#dropPending(entry);

        return new Promise((resolve, reject) => {
            const download = this.#download(id, entry);
            const pending = { download, settled: false, holdTimer: null };
            entry.pending = pending;

            const settle = (error) => {
                if (pending.settled) return;
                pending.settled = true;
                clearTimeout(startTimer);
                if (error) {
                    this.#dropPending(entry);
                    reject(error);
                    return;
                }
                // Nobody connected in time (song skipped meanwhile): stop the download.
                pending.holdTimer = setTimeout(() => this.#dropPending(entry), holdMs);
                pending.holdTimer.unref?.();
                resolve(relayUrl);
            };

            const startTimer = setTimeout(() => settle(new Error(`No audio received within ${Math.round(timeoutMs / 1000)}s`)), timeoutMs);
            download.onFirstChunk(() => settle(null));
            download.done.then((error) => {
                if (error) settle(error);
                else settle(download.size ? null : new Error('The source returned no audio'));
            });
        });
    }

    #dropPending(entry) {
        const pending = entry.pending;
        if (!pending) return;
        entry.pending = null;
        clearTimeout(pending.holdTimer);
        pending.download.kill();
    }

    /**
     * Spawn yt-dlp for an entry. `done` resolves with an Error on failure, null on success.
     * Decouples the download from playback consumption rate so yt-dlp never stalls on YouTube TCP window.
     */
    #download(id, entry) {
        const child = this.spawn(this.binary, this.args(entry), { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
        let stderr = '';
        let received = false;
        const chunks = [];
        let totalSize = 0;
        let error = null;
        let closed = false;
        let activeRes = null;
        let killTimer = null;
        let clientStart = 0;
        let clientEnd = null;
        let remainingToSkip = 0;
        let chunkIndex = 0;
        let bytesSent = 0;
        let pumping = false;
        let waitingDrain = false;
        const firstChunkCallbacks = [];

        child.stderr.setEncoding('utf8').on('data', (data) => (stderr += data));

        const kill = () => {
            clearTimeout(killTimer);
            if (child.exitCode === null && !child.killed) child.kill('SIGKILL');
        };

        const onDrain = () => {
            waitingDrain = false;
            pump();
        };

        const pump = () => {
            if (pumping || waitingDrain || !activeRes || activeRes.destroyed || activeRes.writableEnded) return;
            pumping = true;
            try {
                while (chunkIndex < chunks.length && remainingToSkip > 0) {
                    const chunk = chunks[chunkIndex];
                    if (remainingToSkip >= chunk.length) {
                        remainingToSkip -= chunk.length;
                        chunkIndex++;
                    } else {
                        break;
                    }
                }

                while (chunkIndex < chunks.length) {
                    let chunk = chunks[chunkIndex];
                    if (remainingToSkip > 0) {
                        chunk = chunk.subarray(remainingToSkip);
                        remainingToSkip = 0;
                    }

                    if (clientEnd !== null) {
                        const maxAllowed = (clientEnd - clientStart + 1) - bytesSent;
                        if (maxAllowed <= 0) {
                            activeRes.end();
                            return;
                        }
                        if (chunk.length > maxAllowed) {
                            chunk = chunk.subarray(0, maxAllowed);
                        }
                    }

                    bytesSent += chunk.length;
                    chunkIndex++;
                    const ok = activeRes.write(chunk);
                    if (!ok) {
                        waitingDrain = true;
                        activeRes.once('drain', onDrain);
                        return;
                    }

                    if (clientEnd !== null && bytesSent >= (clientEnd - clientStart + 1)) {
                        activeRes.end();
                        return;
                    }
                }

                if (closed) {
                    if (error) {
                        activeRes.destroy(error);
                    } else {
                        activeRes.end();
                    }
                }
            } finally {
                pumping = false;
            }
        };

        child.stdout.on('data', (chunk) => {
            received = true;
            chunks.push(chunk);
            totalSize += chunk.length;
            while (firstChunkCallbacks.length) firstChunkCallbacks.shift()();
            pump();
        });

        const done = new Promise((resolve) => {
            child.on('error', (err) => {
                const failure = Object.assign(new Error(err.message), { stderr: err.message });
                this.errors.set(id, failure);
                error = failure;
                closed = true;
                if (activeRes && !activeRes.destroyed && !activeRes.writableEnded) {
                    activeRes.destroy(failure);
                }
                resolve(failure);
            });
            child.on('close', (code, signal) => {
                closed = true;
                if ((code && !signal) || (!received && !signal)) {
                    const failure = Object.assign(
                        new Error(received ? ytdlp.cleanError(stderr) : (ytdlp.cleanError(stderr) || 'The source returned no audio')),
                        { stderr }
                    );
                    this.errors.set(id, failure);
                    log.warn(`Download ${received ? 'interrupted' : 'failed'}: ${failure.message}`);
                    error = failure;
                    if (activeRes && !activeRes.destroyed && !activeRes.writableEnded) {
                        activeRes.destroy(failure);
                    }
                    resolve(failure);
                } else {
                    if (received) this.errors.delete(id);
                    pump();
                    resolve(null);
                }
            });
        });

        const onFirstChunk = (fn) => {
            if (received) fn();
            else firstChunkCallbacks.push(fn);
        };

        const attach = (res, options = {}) => {
            clearTimeout(killTimer);
            activeRes = res;
            clientStart = options.start || 0;
            clientEnd = options.end ?? null;
            remainingToSkip = clientStart;
            chunkIndex = 0;
            bytesSent = 0;
            waitingDrain = false;

            res.on('close', () => {
                if (activeRes === res) {
                    activeRes = null;
                    clearTimeout(killTimer);
                    // Retain download in memory for 5s to allow ffmpeg reconnection with Range header
                    killTimer = setTimeout(() => {
                        if (!activeRes && !closed) kill();
                    }, 5000);
                    killTimer.unref?.();
                }
            });
            res.on('error', (err) => {
                log.debug(`Relay response error: ${err.message}`);
            });
            pump();
        };

        return {
            child,
            done,
            kill,
            attach,
            onFirstChunk,
            get size() { return totalSize; },
            get closed() { return closed; },
            get error() { return error; },
        };
    }

    #handle(req, res) {
        const id = req.url.slice(1).split('?')[0];
        const entry = this.entries.get(id);
        if (!entry || req.method !== 'GET') return res.writeHead(404).end();

        const pending = entry.pending;
        entry.pending = null;
        let download;
        if (pending) {
            clearTimeout(pending.holdTimer);
            download = pending.download;
            entry.activeDownload = download;
        } else {
            const rawRange = parseRange(req.headers.range);
            // Reconnection request: reuse existing active download if valid
            if (rawRange && rawRange.start > 0 && entry.activeDownload && !entry.activeDownload.error) {
                download = entry.activeDownload;
            } else {
                download = this.#download(id, entry);
                entry.activeDownload = download;
            }
        }

        const total = download.closed ? download.size : null;
        const range = parseRange(req.headers.range, total);

        if (range) {
            if (total !== null && range.start >= total) {
                return res.writeHead(416, {
                    'Content-Range': `bytes */${total}`,
                    'Accept-Ranges': 'bytes',
                    'Cache-Control': 'no-store',
                }).end();
            }

            const end = range.end !== null ? range.end : (total ? total - 1 : null);
            const contentRange = `bytes ${range.start}-${end !== null ? end : '*'}/${total !== null ? total : '*'}`;
            const headers = {
                'Content-Type': 'application/octet-stream',
                'Content-Range': contentRange,
                'Accept-Ranges': 'bytes',
                'Cache-Control': 'no-store',
            };
            if (end !== null && range.start !== null) {
                headers['Content-Length'] = String(end - range.start + 1);
            }
            res.writeHead(206, headers);
            download.attach(res, { start: range.start, end });
        } else {
            const headers = {
                'Content-Type': 'application/octet-stream',
                'Accept-Ranges': 'bytes',
                'Cache-Control': 'no-store',
            };
            if (download.closed) {
                headers['Content-Length'] = String(download.size);
            }
            res.writeHead(200, headers);
            download.attach(res, { start: 0, end: null });
        }
    }

    #forget(id) {
        const entry = this.entries.get(id);
        if (!entry) return;
        clearTimeout(entry.timer);
        this.#dropPending(entry);
        if (entry.activeDownload) {
            entry.activeDownload.kill();
            entry.activeDownload = null;
        }
        this.entries.delete(id);
        this.errors.delete(id);
        if (entry.infoFile) fsp.rm(entry.infoFile, { force: true }).catch(() => undefined);
    }

    close() {
        for (const id of [...this.entries.keys()]) this.#forget(id);
        this.server?.closeAllConnections?.();
        this.server?.close();
        fs.rmSync(this.dir, { recursive: true, force: true });
    }

    /** Remove folders left by earlier runs (called before shards start). */
    static cleanAll(dataDir = config.dataDir) {
        fs.rmSync(path.join(dataDir, 'relay'), { recursive: true, force: true });
    }
}

let shared = null;
const getRelay = () => (shared ??= new StreamRelay());

module.exports = { StreamRelay, getRelay, trimInfo, parseRange };
