'use strict';

const path = require('node:path');
require('./helpers');
process.env.YTDLP_PATH = path.join(__dirname, 'fixtures', 'fake-ytdlp.js');

const test = require('node:test');
const assert = require('node:assert/strict');
const http = require('node:http');
const { StreamRelay } = require('../src/music/relay');

const skip = false;

function fetchBytes(url, headers = {}) {
    return new Promise((resolve, reject) => {
        const u = new URL(url);
        http.get({ hostname: u.hostname, port: u.port, path: u.pathname, headers }, (res) => {
            let size = 0;
            res.on('data', (chunk) => (size += chunk.length));
            res.on('end', () => resolve({ status: res.statusCode, size, headers: res.headers }));
            res.on('error', reject);
            res.on('close', () => { if (!res.complete) reject(new Error('aborted')); });
        }).on('error', reject);
    });
}

function spyRelay() {
    const calls = [];
    const { spawn } = require('node:child_process');
    const relay = new StreamRelay({
        dir: path.join(process.env.DATA_DIR, `relay-${Math.random()}`),
        spawnImpl: (bin, args, opts) => {
            calls.push(args);
            if (process.platform === 'win32' && bin.endsWith('.js')) {
                return spawn(process.execPath, [bin, ...args], opts);
            }
            return spawn(bin, args, opts);
        },
    });
    return { relay, calls };
}

test('the relay downloads the already chosen format with yt-dlp and serves it to ffmpeg', { skip }, async () => {
    const { relay, calls } = spyRelay();
    try {
        const url = await relay.register({ info: { id: 'x', format_id: '251', url: 'https://googlevideo/ok' }, proxy: 'socks5://p:1' });
        assert.match(url, /^http:\/\/127\.0\.0\.1:\d+\/[0-9a-f]{24}$/);
        assert.equal(await relay.warm(url), url);
        const first = await fetchBytes(url);
        assert.equal(first.status, 200);
        assert.equal(first.size, 50_000);
        assert.equal(calls.length, 1, 'the warmed download is reused');
        const args = calls[0];
        assert.equal(args[args.indexOf('-f') + 1], '251');
        assert.ok(args.includes('--load-info-json'));
        assert.equal(args[args.indexOf('--proxy') + 1], 'socks5://p:1', 'any proxy type works because yt-dlp downloads');
        assert.deepEqual(args.slice(args.indexOf('-o'), args.indexOf('-o') + 2), ['-o', '-']);

        const again = await fetchBytes(url);
        assert.equal(again.size, 50_000, 'loop: the same song can be streamed again');
        assert.equal(calls.length, 2);
        assert.equal(relay.errorFor(url), null);
    } finally {
        relay.close();
    }
});

test('a refused source fails during warm() with yt-dlp\'s reason, before playback starts', { skip }, async () => {
    const { relay } = spyRelay();
    try {
        const url = await relay.register({ info: { id: 'x', format_id: '251', url: 'https://googlevideo/fail' } });
        await assert.rejects(relay.warm(url), /HTTP Error 403: Forbidden/);
        assert.match(relay.errorFor(url).message, /403/);
        assert.equal(relay.errorFor('http://example.com/x'), null);
    } finally {
        relay.close();
    }
});

test('warm() gives up when no audio arrives, and an unused warmed download is stopped', { skip }, async () => {
    const { relay } = spyRelay();
    try {
        const slow = await relay.register({ url: 'https://cdn/slow.mp3' });
        await assert.rejects(relay.warm(slow, { timeoutMs: 300 }), /No audio received/);

        const unused = await relay.register({ url: 'https://cdn/ok.mp3' });
        await relay.warm(unused, { holdMs: 100 });
        await new Promise((r) => setTimeout(r, 300));
        const entry = [...relay.entries.values()].find((e) => e.url === 'https://cdn/ok.mp3');
        assert.equal(entry.pending, null, 'the held download was dropped');
        assert.equal((await fetchBytes(unused)).size, 50_000, 'and a new one starts when ffmpeg connects later');
    } finally {
        relay.close();
    }
});

test('unknown relay ids return 404', { skip }, async () => {
    const { relay } = spyRelay();
    try {
        await relay.start();
        assert.equal((await fetchBytes(`http://127.0.0.1:${relay.port}/nope`)).status, 404);
        await assert.rejects(relay.warm(`http://127.0.0.1:${relay.port}/nope`), /Unknown relay stream/);
    } finally {
        relay.close();
    }
});

test('stream info is trimmed to the chosen format before it is written', () => {
    const { trimInfo } = require('../src/music/relay');
    const info = {
        id: 'x', format_id: '251', url: 'u',
        formats: [{ format_id: '140' }, { format_id: '251', url: 'u', http_headers: { a: 1 } }],
        automatic_captions: { en: [] }, subtitles: {}, thumbnails: [], description: 'long',
    };
    const trimmed = trimInfo(info);
    assert.deepEqual(trimmed.formats, [{ format_id: '251', url: 'u', http_headers: { a: 1 } }]);
    for (const key of ['automatic_captions', 'subtitles', 'thumbnails', 'description']) assert.equal(key in trimmed, false);
    assert.equal(info.formats.length, 2, 'the cached info is not modified');
});

test('each process uses its own relay folder; old streams are evicted beyond the limit', { skip }, async () => {
    const { relay } = spyRelay();
    try {
        assert.ok(new StreamRelay().dir.endsWith(String(process.pid)));
        for (let i = 0; i < 305; i++) await relay.register({ url: `https://cdn/${i}.mp3` });
        assert.equal(relay.entries.size, 300);
        assert.equal([...relay.entries.values()][0].url, 'https://cdn/5.mp3');
    } finally {
        relay.close();
    }
});

test('mid-stream failure destroys the response with error instead of sending a clean end', { skip }, async () => {
    const { relay } = spyRelay();
    try {
        const url = await relay.register({ info: { id: 'x', format_id: '251', url: 'https://googlevideo/cutoff' } });
        await relay.warm(url);
        await assert.rejects(fetchBytes(url));
        assert.match(relay.errorFor(url).message, /Connection reset by peer/);
    } finally {
        relay.close();
    }
});

test('Range requests return 206 Partial Content and reuse active download for seamless reconnect', { skip }, async () => {
    const { relay, calls } = spyRelay();
    try {
        const url = await relay.register({ info: { id: 'x', format_id: '251', url: 'https://googlevideo/ok' } });
        await relay.warm(url);

        // First connection: starts at 0, receives all 50,000 bytes
        const first = await fetchBytes(url);
        assert.equal(first.status, 200);
        assert.equal(first.size, 50_000);
        assert.equal(first.headers['accept-ranges'], 'bytes');
        assert.equal(calls.length, 1);

        // Reconnect with Range header: byte 20,000 to end
        const reconnect = await fetchBytes(url, { Range: 'bytes=20000-' });
        assert.equal(reconnect.status, 206);
        assert.equal(reconnect.size, 30_000);
        assert.equal(reconnect.headers['content-range'], 'bytes 20000-49999/50000');
        assert.equal(calls.length, 1, 'reconnection reuses the active download without spawning yt-dlp again');

        // Bounded range request: 1,000 to 1,999 (1,000 bytes)
        const slice = await fetchBytes(url, { Range: 'bytes=1000-1999' });
        assert.equal(slice.status, 206);
        assert.equal(slice.size, 1000);
        assert.equal(slice.headers['content-range'], 'bytes 1000-1999/50000');
        assert.equal(calls.length, 1);
    } finally {
        relay.close();
    }
});

test('parseRange unit tests: validates start, end, suffix and error cases', () => {
    const { parseRange } = require('../src/music/relay');
    assert.deepEqual(parseRange('bytes=0-'), { start: 0, end: null });
    assert.deepEqual(parseRange('bytes=100-200', 500), { start: 100, end: 200 });
    assert.deepEqual(parseRange('bytes=100-', 500), { start: 100, end: 499 });
    assert.deepEqual(parseRange('bytes=-50', 500), { start: 450, end: 499 });
    assert.equal(parseRange('invalid'), null);
    assert.equal(parseRange('bytes=abc-'), null);
    assert.equal(parseRange('bytes=200-100'), null);
});

test('large audio streams (>8MB) stream completely without backpressure stall', { skip }, async () => {
    const { relay } = spyRelay();
    try {
        const url = await relay.register({ info: { id: 'x', format_id: '251', url: 'https://googlevideo/large' } });
        await relay.warm(url);
        const res = await fetchBytes(url);
        assert.equal(res.status, 200);
        assert.equal(res.size, 10 * 1024 * 1024, 'All 10MB received without stall or truncation');
    } finally {
        relay.close();
    }
});
