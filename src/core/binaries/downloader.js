'use strict';

const { createWriteStream } = require('node:fs');
const fsp = require('node:fs/promises');
const { createHash } = require('node:crypto');
const { Readable, Transform } = require('node:stream');
const { pipeline } = require('node:stream/promises');

const DEFAULT_MAX_BYTES = 512 * 1024 * 1024;
const DEFAULT_TIMEOUT_MS = 10 * 60_000;

/**
 * Stream a download straight to disk while hashing it, so a 30-60 MB binary
 * never sits in memory. Rejects (and removes the partial file) on HTTP errors,
 * size limits or checksum mismatch. Returns { bytes, sha256 }.
 */
async function downloadToFile(url, destination, {
    minBytes = 1024 * 1024,
    maxBytes = DEFAULT_MAX_BYTES,
    sha256 = null,
    timeoutMs = DEFAULT_TIMEOUT_MS,
} = {}) {
    const response = await fetch(url, { redirect: 'follow', signal: AbortSignal.timeout(timeoutMs) });
    if (!response.ok) throw new Error(`HTTP ${response.status} for ${url}`);

    const hash = createHash('sha256');
    let bytes = 0;
    const meter = new Transform({
        transform(chunk, encoding, callback) {
            bytes += chunk.length;
            if (bytes > maxBytes) {
                callback(new Error(`Download from ${url} exceeds ${maxBytes} bytes`));
                return;
            }
            hash.update(chunk);
            callback(null, chunk);
        },
    });

    try {
        await pipeline(Readable.fromWeb(response.body), meter, createWriteStream(destination));
    } catch (error) {
        await fsp.rm(destination, { force: true });
        throw error;
    }
    if (bytes < minBytes) {
        await fsp.rm(destination, { force: true });
        throw new Error(`Download from ${url} is too small (${bytes} bytes)`);
    }
    const digest = hash.digest('hex');
    if (sha256 && digest !== String(sha256).toLowerCase()) {
        await fsp.rm(destination, { force: true });
        throw new Error(`Checksum mismatch for ${url} (expected ${sha256}, got ${digest})`);
    }
    return { bytes, sha256: digest };
}

module.exports = { downloadToFile };
