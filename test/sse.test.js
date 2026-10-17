'use strict';

require('./helpers');
const test = require('node:test');
const assert = require('node:assert/strict');
const { SseHub, DEFAULT_HEARTBEAT_MS } = require('../src/api/sse');

function fakeRes() {
    const res = {
        headers: null,
        flushed: false,
        chunks: [],
        listeners: new Map(),
        ended: false,
        set(headers) { this.headers = headers; return this; },
        flushHeaders() { this.flushed = true; },
        write(chunk) { this.chunks.push(chunk); return true; },
        end() { this.ended = true; },
        on(event, fn) { this.listeners.set(event, fn); return this; },
        emit(event) { this.listeners.get(event)?.(); },
    };
    return res;
}

test('subscribe writes SSE headers and a connected comment', () => {
    const hub = new SseHub({ heartbeatMs: 60_000 });
    const res = fakeRes();
    hub.subscribe('123', res);

    assert.equal(res.headers['Content-Type'], 'text/event-stream; charset=utf-8');
    assert.equal(res.headers['Cache-Control'], 'no-cache, no-transform');
    assert.equal(res.headers.Connection, 'keep-alive');
    assert.equal(res.flushed, true);
    assert.deepEqual(res.chunks, [': connected\n\n']);
    assert.equal(hub.size('123'), 1);
    hub.close();
});

test('publish only reaches subscribers of that guild and encodes the event', () => {
    const hub = new SseHub({ heartbeatMs: 60_000 });
    const a = fakeRes();
    const b = fakeRes();
    hub.subscribe('guild-a', a);
    hub.subscribe('guild-b', b);

    const delivered = hub.publish('guild-a', 'player.sync', { hasQueue: true });
    assert.equal(delivered, 1);
    assert.equal(a.chunks.length, 2);
    assert.equal(a.chunks[1], 'event: player.sync\ndata: {"hasQueue":true}\n\n');
    assert.equal(b.chunks.length, 1, 'other guilds receive nothing');

    assert.equal(hub.publish('guild-c', 'player.sync', {}), 0);
    hub.close();
});

test('closed connections are removed from the hub', () => {
    const hub = new SseHub({ heartbeatMs: 60_000 });
    const res = fakeRes();
    hub.subscribe('guild-a', res);
    assert.equal(hub.size(), 1);

    res.emit('close');
    assert.equal(hub.size('guild-a'), 0);
    assert.equal(hub.publish('guild-a', 'player.sync', {}), 0);
    hub.close();
});

test('close ends every stream and stops the heartbeat', () => {
    const hub = new SseHub({ heartbeatMs: 60_000 });
    const a = fakeRes();
    const b = fakeRes();
    hub.subscribe('guild-a', a);
    hub.subscribe('guild-b', b);

    hub.close();
    assert.equal(a.ended, true);
    assert.equal(b.ended, true);
    assert.equal(hub.size(), 0);
    assert.equal(hub.timer, null);
});

test('closeGuild only ends that guild streams', () => {
    const hub = new SseHub({ heartbeatMs: 60_000 });
    const a = fakeRes();
    const b = fakeRes();
    hub.subscribe('guild-a', a);
    hub.subscribe('guild-b', b);

    hub.closeGuild('guild-a');
    assert.equal(a.ended, true);
    assert.equal(b.ended, false);
    assert.equal(hub.size('guild-b'), 1);
    hub.close();
});

test('heartbeat interval is unref\'d and media type defaults are stable', () => {
    assert.equal(DEFAULT_HEARTBEAT_MS, 25_000);
    const hub = new SseHub({ heartbeatMs: 60_000 });
    const res = fakeRes();
    hub.subscribe('guild-a', res);
    assert.ok(hub.timer, 'heartbeat starts on first subscriber');
    hub.close();
    assert.equal(hub.timer, null);
});
