'use strict';

require('./helpers');
const test = require('node:test');
const assert = require('node:assert/strict');
const { ShardProxy } = require('../src/api/shard-proxy');

/**
 * Two simulated shards. `broadcastEval` mirrors discord.js: it runs the given
 * script in each target shard with (client, context) and returns the results.
 */
function makeCluster({ shard0Guilds = [], shard1Guilds = [], ops = null, slow = false } = {}) {
    const opsFor = () => (ops ? { apiOps: ops } : null);
    const shards = [
        { guilds: { cache: new Map() }, music: opsFor(), shard: { ids: [0] } },
        { guilds: { cache: new Map() }, music: opsFor(), shard: { ids: [1] } },
    ];
    for (const id of shard0Guilds) shards[0].guilds.cache.set(id, { id });
    for (const id of shard1Guilds) shards[1].guilds.cache.set(id, { id });

    const calls = { locate: 0, dispatch: 0, contexts: [] };
    shards[0].shard.broadcastEval = (script, { shard, context } = {}) => {
        const targets = shard === undefined ? [0, 1] : [shard];
        if (context && context.op) {
            calls.dispatch++;
            calls.contexts.push(context);
        } else {
            calls.locate++;
        }
        if (slow) return new Promise(() => {});
        return Promise.all(targets.map((index) => script(shards[index], context)));
    };

    return { client: shards[0], calls };
}

test('guilds on the API shard run locally without IPC', async () => {
    const ops = { run: async (op, payload) => ({ op, payload }) };
    const { client, calls } = makeCluster({ shard0Guilds: ['111111111111111111'], ops });
    const proxy = new ShardProxy(client);

    const result = await proxy.run('111111111111111111', 'ping', { a: 1 });
    assert.deepEqual(result, { ok: true, data: { op: 'ping', payload: { a: 1 } } });
    assert.equal(calls.locate + calls.dispatch, 0, 'no broadcastEval for local guilds');
});

test('guilds on another shard are located and dispatched through broadcastEval', async () => {
    const ops = {
        seen: null,
        run: async (op, payload) => {
            ops.seen = { op, payload };
            return { players: 2 };
        },
    };
    const { client, calls } = makeCluster({ shard1Guilds: ['222222222222222222'], ops });
    const proxy = new ShardProxy(client);

    const first = await proxy.run('222222222222222222', 'player.state', { withQueue: true });
    assert.deepEqual(first, { ok: true, data: { players: 2 } });
    assert.deepEqual(ops.seen, { op: 'player.state', payload: { withQueue: true } });

    // Second call reuses the cached shard id: no extra locate broadcast.
    const second = await proxy.run('222222222222222222', 'player.state', null);
    assert.deepEqual(second, { ok: true, data: { players: 2 } });
    assert.equal(calls.locate, 1, 'shard lookup is cached');
    assert.equal(calls.dispatch, 2);
    assert.deepEqual(JSON.parse(JSON.stringify(calls.contexts[0])), {
        guildId: '222222222222222222',
        op: 'player.state',
        payload: { withQueue: true },
    }, 'context survives JSON serialization');
});

test('unknown guilds report guild_not_found', async () => {
    const { client } = makeCluster();
    const proxy = new ShardProxy(client);
    const result = await proxy.run('333333333333333333', 'ping');
    assert.equal(result.ok, false);
    assert.equal(result.code, 'guild_not_found');
});

test('shard timeouts fail fast with shard_timeout', async () => {
    const { client } = makeCluster({ shard1Guilds: ['444444444444444444'], slow: true });
    const proxy = new ShardProxy(client, { timeoutMs: 20 });
    const result = await proxy.run('444444444444444444', 'ping');
    assert.equal(result.ok, false);
    assert.equal(result.code, 'shard_timeout');
});

test('operation errors keep their code and message', async () => {
    const ops = {
        run: async () => {
            const error = new Error('You must be in a voice channel');
            error.code = 'not_in_voice';
            throw error;
        },
    };
    const { client } = makeCluster({ shard1Guilds: ['555555555555555555'], ops });
    const proxy = new ShardProxy(client);
    const result = await proxy.run('555555555555555555', 'control.pause');
    assert.equal(result.ok, false);
    assert.equal(result.code, 'not_in_voice');
    assert.match(result.message, /voice channel/);
});

test('operation status travels through the shard boundary (RFC 7807 mapping)', async () => {
    const ops = {
        run: async () => {
            const error = new Error('No queue found');
            error.code = 'NO_QUEUE';
            error.status = 404;
            throw error;
        },
    };
    const { client } = makeCluster({ shard1Guilds: ['777777777777777777'], ops });
    const proxy = new ShardProxy(client);
    const result = await proxy.run('777777777777777777', 'queue.clear');
    assert.equal(result.ok, false);
    assert.equal(result.status, 404);
    assert.equal(result.code, 'NO_QUEUE');
});

test('missing music system on the owning shard reports not_ready', async () => {
    const { client } = makeCluster({ shard1Guilds: ['666666666666666666'] });
    const proxy = new ShardProxy(client);
    const result = await proxy.run('666666666666666666', 'ping');
    assert.equal(result.ok, false);
    assert.equal(result.code, 'not_ready');
});

test('the cached shard id expires after cacheTtlMs and is looked up again', async (t) => {
    t.mock.timers.enable({ apis: ['Date'] });
    const ops = { run: async (op) => ({ op }) };
    const { client, calls } = makeCluster({ shard1Guilds: ['888888888888888888'], ops });
    const proxy = new ShardProxy(client, { cacheTtlMs: 30_000 });

    await proxy.run('888888888888888888', 'ping');
    await proxy.run('888888888888888888', 'ping');
    assert.equal(calls.locate, 1, 'second call reuses the cached shard id');

    t.mock.timers.tick(30_001);
    await proxy.run('888888888888888888', 'ping');
    assert.equal(calls.locate, 2, 'expired cache entry forces a fresh locate');
    assert.equal(calls.dispatch, 3);
});
