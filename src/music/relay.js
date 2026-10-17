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
        const entry = { proxy, url, infoFile: null, format: null, timer: null, pending: null };
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
            const pending = { download, chunks: [], size: 0, settled: false, holdTimer: null };
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
            download.child.stdout.on('data', pending.collect = (chunk) => {
                pending.chunks.push(chunk);
                pending.size += chunk.length;
                if (pending.size > 8 * 1024 * 1024) download.child.stdout.pause();
                settle(null);
            });
            download.done.then((error) => {
                if (error) settle(error);
                else settle(pending.size ? null : new Error('The source returned no audio'));
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
     */
    #download(id, entry) {
        const child = this.spawn(this.binary, this.args(entry), { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
        let stderr = '';
        let received = false;
        child.stderr.setEncoding('utf8').on('data', (data) => (stderr += data));
        child.stdout.once('data', () => (received = true));
        const done = new Promise((resolve) => {
            child.on('error', (error) => {
                const failure = Object.assign(new Error(error.message), { stderr: error.message });
                this.errors.set(id, failure);
                resolve(failure);
            });
            child.on('close', (code, signal) => {
                if (code && !signal) {
                    const failure = Object.assign(new Error(ytdlp.cleanError(stderr)), { stderr });
                    this.errors.set(id, failure);
                    log.warn(`Download ${received ? 'interrupted' : 'failed'}: ${failure.message}`);
                    resolve(failure);
                } else {
                    if (received) this.errors.delete(id);
                    resolve(null);
                }
            });
        });
        const kill = () => { if (child.exitCode === null && !child.killed) child.kill('SIGKILL'); };
        return { child, done, kill };
    }

    #handle(req, res) {
        const id = req.url.slice(1);
        const entry = this.entries.get(id);
        if (!entry || req.method !== 'GET') return res.writeHead(404).end();
        res.writeHead(200, { 'Content-Type': 'application/octet-stream', 'Cache-Control': 'no-store' });

        // Use the download started by warm() when there is one, otherwise start a new one
        // (e.g. the same song again with loop enabled).
        const pending = entry.pending;
        entry.pending = null;
        let download;
        if (pending) {
            clearTimeout(pending.holdTimer);
            download = pending.download;
            download.child.stdout.off('data', pending.collect);
            for (const chunk of pending.chunks) res.write(chunk);
            // pipe() resumes the stream if it was paused at the buffer limit.
        } else {
            download = this.#download(id, entry);
        }
        download.child.stdout.pipe(res);
        res.on('close', download.kill);
        download.done.then(() => res.end());
    }

    #forget(id) {
        const entry = this.entries.get(id);
        if (!entry) return;
        clearTimeout(entry.timer);
        this.#dropPending(entry);
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

module.exports = { StreamRelay, getRelay, trimInfo };
