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
