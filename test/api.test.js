'use strict';

// Must exist before config.js loads (dotenv never overrides real environment).
process.env.API_SECRET = 'test-secret-0123456789abcdef0123456789abcdef';

require('./helpers');
const test = require('node:test');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const { once } = require('node:events');
const express = require('express');
const { ids } = require('./helpers');
const { createRouter } = require('../src/api/v1/router');
const { requireAuth, safeEqual } = require('../src/api/middleware/auth');
const { getSseHub } = require('../src/api/sse');

const SECRET = process.env.API_SECRET;
const authHeaders = { authorization: `Bearer ${SECRET}` };
const playHeaders = { ...authHeaders, 'content-type': 'application/json', 'x-user-discord-id': ids.user };

function fakeRes() {
    return {
        code: null,
        body: null,
        typeHeader: null,
        status(value) { this.code = value; return this; },
        json(value) { this.body = value; return this; },
        type(value) { this.typeHeader = value; return this; },
    };
}

function runAuth(req) {
    const res = fakeRes();
    let nextCalled = false;
    requireAuth(req, res, () => { nextCalled = true; });
    return { res, nextCalled };
}

/**
 * Express app with the real v1 router over a fake shard: the guild lives on
 * this "shard", so ShardProxy takes the local fast path and every op lands in
 * `calls` without any IPC.
 */
async function startApi(opsHandler = null) {
    const calls = [];
    const client = {
        music: {
            apiOps: {
                run: async (op, payload) => {
                    calls.push({ op, payload });
                    return opsHandler ? opsHandler(op, payload) : { op, payload };
                },
            },
        },
        guilds: { cache: new Map([[ids.guild, {}]]) },
        shard: { ids: [0], broadcastEval: async () => [null] },
    };

    const app = express();
    app.use(express.json());
    const router = createRouter(client);
    app.use('/api/v1', router);
    app.use('/api', router);

    const server = app.listen(0);
    await once(server, 'listening');
    const base = `http://127.0.0.1:${server.address().port}`;
    return { base, calls, close: () => new Promise((resolve) => server.close(resolve)) };
}

test('requireAuth fails closed and compares tokens without early length leaks', () => {
    const bare = runAuth({ headers: {} });
    assert.equal(bare.res.code, 401);
    assert.equal(bare.res.body.code, 'UNAUTHORIZED');
    assert.equal(bare.nextCalled, false);

    const realTiming = crypto.timingSafeEqual;
    let comparisons = 0;
    crypto.timingSafeEqual = (...args) => {
        comparisons++;
        return realTiming(...args);
    };
    try {
        // A wrong-length token is rejected before any constant-time compare.
        const short = runAuth({ headers: { authorization: 'Bearer short' } });
        assert.equal(short.res.code, 401);
        assert.equal(comparisons, 0, 'length mismatch must not reach timingSafeEqual');

        // A same-length wrong token goes through exactly one constant-time compare.
        const wrong = runAuth({ headers: { authorization: `Bearer ${'x'.repeat(SECRET.length)}` } });
        assert.equal(wrong.res.code, 401);
        assert.equal(comparisons, 1);

        // The configured secret is accepted.
        const ok = runAuth({ headers: { authorization: `Bearer ${SECRET}` } });
        assert.equal(ok.nextCalled, true);
        assert.equal(ok.res.code, null);
    } finally {
        crypto.timingSafeEqual = realTiming;
    }
});

test('safeEqual tolerates non-strings and length mismatches', () => {
    assert.equal(safeEqual(SECRET, SECRET), true);
    assert.equal(safeEqual(SECRET, 'x'.repeat(SECRET.length)), false);
    assert.equal(safeEqual(SECRET, 'short'), false);
    assert.equal(safeEqual(undefined, SECRET), false);
    assert.equal(safeEqual(Buffer.from(SECRET), SECRET), false);
});

test('the v1 router answers 401 problem+json without a valid bearer token', async () => {
    const api = await startApi();
    try {
        const missing = await fetch(`${api.base}/api/v1/guilds/${ids.guild}/player`);
        assert.equal(missing.status, 401);
        assert.match(missing.headers.get('content-type'), /application\/problem\+json/);
        const body = await missing.json();
        assert.equal(body.code, 'UNAUTHORIZED');
        assert.equal(body.status, 401);

        const wrong = await fetch(`${api.base}/api/v1/guilds/${ids.guild}/player`, {
            headers: { authorization: `Bearer ${'x'.repeat(SECRET.length)}` },
        });
        assert.equal(wrong.status, 401);
        assert.equal(api.calls.length, 0, 'rejected requests never reach the ops layer');

        const ok = await fetch(`${api.base}/api/v1/guilds/${ids.guild}/player`, { headers: authHeaders });
        assert.equal(ok.status, 200);
        assert.deepEqual(await ok.json(), { op: 'player.state', payload: { guildId: ids.guild } });
    } finally {
        await api.close();
    }
});

test('GET player empty matches the shared TypeScript/Rust contract fixture', async () => {
    const fixture = JSON.parse(require('node:fs').readFileSync(
        require('node:path').join(__dirname, 'fixtures/api-contract/player-empty.json'), 'utf8',
    ));
    const api = await startApi(async (op) => {
        assert.equal(op, 'player.state');
        return fixture;
    });
    try {
        const response = await fetch(`${api.base}/api/v1/guilds/${ids.guild}/player`, { headers: authHeaders });
        assert.equal(response.status, 200);
        assert.deepEqual(await response.json(), fixture);
    } finally {
        await api.close();
    }
});

test('invalid player guild uses the shared TypeScript/Rust problem status and code', async () => {
    const expected = JSON.parse(require('node:fs').readFileSync(
        require('node:path').join(__dirname, 'fixtures/api-contract/invalid-snowflake.json'), 'utf8',
    ));
    const api = await startApi();
    try {
        const response = await fetch(`${api.base}/api/v1/guilds/invalid/player`, { headers: authHeaders });
        const problem = await response.json();
        assert.deepEqual({ status: response.status, code: problem.code }, expected);
        assert.equal(api.calls.length, 0);
    } finally {
        await api.close();
    }
});

test('queue clear response and missing-queue problem match shared Rust fixtures', async () => {
    const fs = require('node:fs');
    const path = require('node:path');
    const fixture = (name) => JSON.parse(fs.readFileSync(path.join(__dirname, `fixtures/api-contract/${name}.json`), 'utf8'));
    let requests = 0;
    const api = await startApi(async (op) => {
        requests++;
        if (op === 'queue.clear' && requests === 1) return { success: true, cleared: 3 };
        throw Object.assign(new Error('No queue found'), { code: 'NO_QUEUE', status: 404 });
    });
    try {
        const headers = { ...authHeaders, 'x-user-discord-id': ids.user };
        const cleared = await fetch(`${api.base}/api/v1/guilds/${ids.guild}/queue`, { method: 'DELETE', headers });
        assert.equal(cleared.status, 200);
        assert.deepEqual(await cleared.json(), fixture('queue-clear'));

        const missing = await fetch(`${api.base}/api/v1/guilds/${ids.guild}/queue`, { method: 'DELETE', headers });
        const problem = await missing.json();
        assert.deepEqual({ status: missing.status, code: problem.code }, fixture('queue-missing'));
        assert.deepEqual(api.calls.map(({ op }) => op), ['queue.clear', 'queue.clear']);
    } finally {
        await api.close();
    }
});

test('queue remove, reorder and undo responses match shared Rust fixtures', async () => {
    const fs = require('node:fs');
    const path = require('node:path');
    const fixture = (name) => JSON.parse(fs.readFileSync(path.join(__dirname, `fixtures/api-contract/${name}.json`), 'utf8'));
    const api = await startApi(async (op) => {
        if (op === 'queue.removeIndex') return { success: true, removed: { title: 'Song 3' } };
        if (op === 'queue.removeTrack') return { success: true, removed: { title: 'Song 2' } };
        if (op === 'queue.order') return { success: true };
        if (op === 'queue.undo') return { success: true, restored: 3, message: '3 canciones restauradas a la cola.' };
        throw new Error(`Unexpected op ${op}`);
    });
    try {
        const headers = { ...authHeaders, 'x-user-discord-id': ids.user };
        const base = `${api.base}/api/v1/guilds/${ids.guild}`;
        const removedIndex = await fetch(`${base}/queue/2`, { method: 'DELETE', headers });
        const indexBody = await removedIndex.json();
        assert.deepEqual({ status: removedIndex.status, success: indexBody.success, removedTitle: indexBody.removed.title }, fixture('queue-remove-index'));

        const removedTrack = await fetch(`${base}/queue/items/track_id_test_2`, { method: 'DELETE', headers });
        const trackBody = await removedTrack.json();
        assert.deepEqual({ status: removedTrack.status, success: trackBody.success, removedTitle: trackBody.removed.title }, fixture('queue-remove-track'));

        const reordered = await fetch(`${base}/queue/order`, { method: 'PATCH', headers: { ...headers, 'content-type': 'application/json' }, body: JSON.stringify({ trackId: 'track_id_test_4', to: 1 }) });
        assert.deepEqual({ status: reordered.status, success: (await reordered.json()).success }, fixture('queue-order'));

        const undone = await fetch(`${base}/queue/undo`, { method: 'POST', headers });
        const undoBody = await undone.json();
        assert.deepEqual({ status: undone.status, success: undoBody.success, restored: undoBody.restored }, fixture('queue-undo'));
        assert.deepEqual(api.calls.map(({ op }) => op), ['queue.removeIndex', 'queue.removeTrack', 'queue.order', 'queue.undo']);
    } finally {
        await api.close();
    }
});

test('the legacy /api mount keeps working for the dashboard BFF', async () => {
    const api = await startApi();
    try {
        const res = await fetch(`${api.base}/api/guilds/${ids.guild}/player`, { headers: authHeaders });
        assert.equal(res.status, 200);
        assert.equal((await res.json()).op, 'player.state');
    } finally {
        await api.close();
    }
});

test('POST play validates the snowflake before anything else', async () => {
    const api = await startApi();
    try {
        const res = await fetch(`${api.base}/api/v1/guilds/not-a-snowflake/play`, {
            method: 'POST',
            headers: playHeaders,
            body: JSON.stringify({ query: 'lofi beats' }),
        });
        assert.equal(res.status, 400);
        assert.equal((await res.json()).code, 'INVALID_SNOWFLAKE');
        assert.equal(api.calls.length, 0);
    } finally {
        await api.close();
    }
});

test('POST play blocks private/local targets before the proxy and shapes the op payload', async () => {
    const api = await startApi();
    try {
        const url = `${api.base}/api/v1/guilds/${ids.guild}/play`;
        for (const query of ['http://127.0.0.1:3001/stream', 'http://localhost/x', 'http://192.168.1.10/a', 'http://[::1]/x', 'file:///etc/passwd', 'data:text/plain,hi']) {
            const res = await fetch(url, { method: 'POST', headers: playHeaders, body: JSON.stringify({ query }) });
            assert.equal(res.status, 400, query);
            assert.equal((await res.json()).code, 'BLOCKED_URL', query);
        }
        assert.equal(api.calls.length, 0, 'blocked targets never reach the ops layer');

        const good = await fetch(url, { method: 'POST', headers: playHeaders, body: JSON.stringify({ query: 'lofi beats', userId: ids.bot }) });
        assert.equal(good.status, 200);
        assert.equal(api.calls.length, 1);
        assert.equal(api.calls[0].op, 'player.play');
        // The validated X-User-Discord-Id wins over the legacy body userId.
        assert.equal(api.calls[0].payload.userId, ids.user);
        assert.equal(api.calls[0].payload.query, 'lofi beats');
    } finally {
        await api.close();
    }
});

test('POST play enforces the query limit and only accepts dev mode through the trusted route inputs', async () => {
    const api = await startApi();
    try {
        const url = `${api.base}/api/v1/guilds/${ids.guild}/play`;
        const tooLong = await fetch(url, { method: 'POST', headers: playHeaders, body: JSON.stringify({ query: 'x'.repeat(1001) }) });
        const problem = await tooLong.json();
        assert.deepEqual({ status: tooLong.status, code: problem.code, detail: problem.detail }, JSON.parse(require('node:fs').readFileSync(
            require('node:path').join(__dirname, 'fixtures/api-contract/play-query-too-long.json'), 'utf8',
        )));
        assert.equal(api.calls.length, 0);

        const bodyFlag = await fetch(url, { method: 'POST', headers: playHeaders, body: JSON.stringify({ query: 'lofi', devMode: true }) });
        assert.equal(bodyFlag.status, 200);
        assert.equal(api.calls[0].payload.devMode, false);

        const headerFlag = await fetch(url, { method: 'POST', headers: { ...playHeaders, 'x-dev-mode': 'true' }, body: JSON.stringify({ query: 'lofi' }) });
        assert.equal(headerFlag.status, 200);
        assert.equal(api.calls[1].payload.devMode, true);
    } finally {
        await api.close();
    }
});

/** Read the SSE stream until `predicate` matches or the deadline passes. */
async function readUntil(reader, predicate, timeoutMs = 4000) {
    const decoder = new TextDecoder();
    let text = '';
    const deadline = Date.now() + timeoutMs;
    while (!predicate(text) && Date.now() < deadline) {
        const chunk = await Promise.race([
            reader.read(),
            new Promise((resolve) => setTimeout(() => resolve(null), Math.max(50, deadline - Date.now()))),
        ]);
        if (!chunk || chunk.done) break;
        text += decoder.decode(chunk.value, { stream: true });
    }
    return text;
}

test('GET events streams the initial snapshot and later published syncs', async () => {
    const api = await startApi(async () => ({ players: 1, songs: [] }));
    const controller = new AbortController();
    try {
        const response = await fetch(`${api.base}/api/v1/guilds/${ids.guild}/events`, {
            headers: authHeaders,
            signal: controller.signal,
        });
        assert.equal(response.status, 200);
        assert.match(response.headers.get('content-type'), /text\/event-stream/);

        const reader = response.body.getReader();
        const initial = await readUntil(reader, (text) => text.includes('player.sync'));
        assert.match(initial, /: connected/);
        assert.match(initial, /event: player\.sync/);
        assert.match(initial, /"players":1/);

        getSseHub().publish(ids.guild, 'player.sync', { players: 2 });
        const next = await readUntil(reader, (text) => text.includes('"players":2'));
        assert.match(next, /event: player\.sync/);
        assert.match(next, /"players":2/);
    } finally {
        controller.abort();
        getSseHub().close();
        await api.close();
    }
});

test('player controls share response bodies and ordered SSE snapshots with Rust', async () => {
    const player = require('../src/api/v1/ops/player');
    const { t } = require('../src/core/i18n');
    const cases = require('./fixtures/api-contract/player-controls.json');
    const queue = {
        songs: [{ id: 'test-song', name: 'Test Song', duration: 213, url: 'https://example.test/song' }],
        currentTime: 0, paused: false, playing: true, volume: 80, repeatMode: 0, autoplay: false,
        filters: {
            names: [],
            get size() { return this.names.length; },
            set(names) { this.names = names; },
            clear() { this.names = []; },
        },
        pause() { this.paused = true; },
        resume() { this.paused = false; },
        setVolume(volume) { this.volume = volume; },
        seek(seconds) { this.currentTime = seconds; },
        toggleAutoplay() { this.autoplay = !this.autoplay; return this.autoplay; },
    };
    const music = {
        distube: { getQueue: () => queue },
        state: () => ({}),
        translator: () => (key, vars) => t('en', key, vars),
        publishSync: (guildId) => getSseHub().publish(guildId, 'player.sync', player.state(music, { guildId })),
    };
    const api = await startApi((op, payload) => op === 'player.control'
        ? player.control(music, payload)
        : player.state(music, payload));
    const controller = new AbortController();
    const base = `${api.base}/api/v1/guilds/${ids.guild}`;
    const control = (body) => fetch(`${base}/control`, {
        method: 'POST', headers: playHeaders, body: JSON.stringify(body),
    });
    try {
        const events = await fetch(`${base}/events`, {
            headers: authHeaders, signal: AbortSignal.any([controller.signal, AbortSignal.timeout(10000)]),
        });
        assert.equal(events.status, 200);
        const reader = events.body.pipeThrough(new TextDecoderStream()).getReader();
        let pending = '';
        const nextSnapshot = async () => {
            for (;;) {
                const end = pending.indexOf('\n\n');
                if (end >= 0) {
                    const event = pending.slice(0, end);
                    pending = pending.slice(end + 2);
                    if (event.startsWith('event: player.sync\n')) {
                        return JSON.parse(event.split('\n').find((line) => line.startsWith('data: ')).slice(6));
                    }
                } else {
                    const { value, done } = await reader.read();
                    assert.equal(done, false, 'SSE ended before the snapshot');
                    pending += value;
                }
            }
        };
        const expectedState = await nextSnapshot();
        for (const body of [{ action: 'filter', value: {} }, { action: '' }, {}]) {
            const response = await control(body);
            assert.equal(response.status, 400);
            assert.equal((await response.json()).code, 'INVALID_PARAMETER');
        }
        assert.equal(api.calls.filter(({ op }) => op === 'player.control').length, 0);
        for (const { request, response, state } of cases) {
            const label = JSON.stringify(request);
            const result = await control(request);
            assert.equal(result.status, 200, label);
            assert.deepEqual(await result.json(), {
                success: response.success,
                message: response.message ?? t('en', response.messageKey),
            }, label);
            Object.assign(expectedState, state);
            const snapshot = await nextSnapshot();
            assert.deepEqual(snapshot, expectedState, label);
            const rest = await fetch(`${base}/player`, { headers: authHeaders });
            assert.deepEqual(await rest.json(), snapshot, label);
        }
    } finally {
        controller.abort();
        getSseHub().close();
        await api.close();
    }
});

test('GET search/playlists and GET search?type=playlist dispatch to discovery.searchPlaylists', async () => {
    const api = await startApi(async () => ({ query: 'reggaeton', results: [{ id: 'PL123', title: 'Top Hits' }] }));
    try {
        const res1 = await fetch(`${api.base}/api/v1/search/playlists?q=reggaeton&limit=5`, { headers: authHeaders });
        assert.equal(res1.status, 200);
        assert.equal(api.calls.length, 1);
        assert.equal(api.calls[0].op, 'discovery.searchPlaylists');
        assert.equal(api.calls[0].payload.q, 'reggaeton');
        assert.equal(api.calls[0].payload.limit, 5);

        const res2 = await fetch(`${api.base}/api/v1/search?q=rock&type=playlist&limit=8`, { headers: authHeaders });
        assert.equal(res2.status, 200);
        assert.equal(api.calls.length, 2);
        assert.equal(api.calls[1].op, 'discovery.searchPlaylists');
        assert.equal(api.calls[1].payload.q, 'rock');
        assert.equal(api.calls[1].payload.limit, 8);
    } finally {
        await api.close();
    }
});

test('discovery query validation and normalization match the Rust API contract', async () => {
    const api = await startApi(async (_op, payload) => ({ query: payload.q, results: [] }));
    try {
        const invalidUrls = [
            '/search?limit=0',
            '/search?limit=16',
            '/search?limit=1.5',
            '/search?type=unknown',
            `/search?q=${'x'.repeat(501)}`,
            `/search/suggest?q=${'x'.repeat(501)}`,
            '/search/playlists?limit=16',
            `/guilds/${ids.guild}/top-songs?limit=101`,
            `/guilds/${ids.guild}/voice-status?userId=+++`,
        ];
        for (const path of invalidUrls) {
            const response = await fetch(`${api.base}/api/v1${path}`, { headers: authHeaders });
            const problem = await response.json();
            assert.equal(response.status, 400, path);
            assert.equal(problem.code, 'INVALID_PARAMETER', path);
        }
        assert.equal(api.calls.length, 0, 'invalid discovery inputs never reach the operation');

        const response = await fetch(`${api.base}/api/v1/search?q=+lofi+&limit=1.0`, { headers: authHeaders });
        assert.equal(response.status, 200);
        assert.deepEqual(await response.json(), { query: 'lofi', results: [] });
        assert.deepEqual(api.calls[0], { op: 'discovery.search', payload: { q: 'lofi', limit: 1 } });
    } finally {
        await api.close();
    }
});
