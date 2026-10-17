'use strict';

require('./helpers');
const test = require('node:test');
const assert = require('node:assert/strict');
const {
    schemas,
    validateBody,
    validateParams,
    validateQuery,
    inspectTarget,
    isPrivateHostname,
} = require('../src/api/middleware/validate');

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

function run(middleware, req) {
    const res = fakeRes();
    let nextCalled = false;
    middleware(req, res, () => { nextCalled = true; });
    return { res, nextCalled };
}

test('guildId params accept snowflakes and reject anything else', () => {
    const ok = run(validateParams(schemas.params.guildId, 'INVALID_SNOWFLAKE'), { params: { guildId: '123456789012345678' } });
    assert.equal(ok.nextCalled, true);
    assert.equal(ok.res.body, null);

    for (const bad of ['123', 'abc', '12345678901234567a', '', '1'.repeat(21)]) {
        const out = run(validateParams(schemas.params.guildId, 'INVALID_SNOWFLAKE'), { params: { guildId: bad } });
        assert.equal(out.nextCalled, false, `expected ${bad} to fail`);
        assert.equal(out.res.code, 400);
        assert.equal(out.res.body.code, 'INVALID_SNOWFLAKE');
        assert.equal(out.res.typeHeader, 'application/problem+json');
    }
});

test('trackId and queue order schemas enforce bounds', () => {
    const good = run(validateBody(schemas.body.order), {
        body: { trackId: 'kZ9_x-12Ab', to: 3 },
    });
    assert.equal(good.nextCalled, true);
    assert.equal(good.res.body, null);

    const badTrack = run(validateBody(schemas.body.order), { body: { trackId: 'short', to: 3 } });
    assert.equal(badTrack.res.code, 400);

    const badTo = run(validateBody(schemas.body.order), { body: { trackId: 'kZ9_x-12Ab', to: 0 } });
    assert.equal(badTo.res.code, 400);
});

test('play body trims the query and enforces limits', () => {
    const ok = run(validateBody(schemas.body.play), { body: { query: '  daft punk  ', next: true } });
    assert.equal(ok.nextCalled, true);
    assert.equal(ok.res.body, null);

    const empty = run(validateBody(schemas.body.play), { body: { query: '   ' } });
    assert.equal(empty.res.code, 400);
    assert.match(empty.res.body.detail, /required/);

    const tooLong = run(validateBody(schemas.body.play), { body: { query: 'x'.repeat(1001) } });
    assert.equal(tooLong.res.code, 400);

    const badVoice = run(validateBody(schemas.body.play), { body: { query: 'ok', voiceChannelId: 'nope' } });
    assert.equal(badVoice.res.code, 400);
});

test('search query caps limit at 15 and top songs at 50', () => {
    const ok = run(validateQuery(schemas.query.search), { query: { q: 'pink floyd', limit: '10' } });
    assert.equal(ok.nextCalled, true);

    const tooMany = run(validateQuery(schemas.query.search), { query: { limit: '99' } });
    assert.equal(tooMany.res.code, 400);

    const badTimeframe = run(validateQuery(schemas.query.topSongs), { query: { timeframe: 'yesterday' } });
    assert.equal(badTimeframe.res.code, 400);

    const okTop = run(validateQuery(schemas.query.topSongs), { query: { timeframe: 'month' } });
    assert.equal(okTop.nextCalled, true);
});

test('plain search text is allowed through the SSRF guard', () => {
    assert.equal(inspectTarget('daft punk one more time'), null);
    assert.equal(inspectTarget('https://www.youtube.com/watch?v=dQw4w9WgXcQ'), null);
    assert.equal(inspectTarget('https://open.spotify.com/track/abc'), null);
    assert.equal(inspectTarget('https://172.32.0.1/not-private'), null);
});

test('local file and script schemes are blocked', () => {
    for (const target of ['file:///etc/passwd', 'data:text/html,<b>x', 'javascript:alert(1)']) {
        const out = inspectTarget(target);
        assert.equal(out?.code, 'BLOCKED_URL', `expected ${target} blocked`);
    }
});

test('loopback, private, link-local and metadata hosts are blocked', () => {
    const blocked = [
        'http://localhost:8080/play',
        'http://sub.localhost/play',
        'https://printer.local/play',
        'http://service.internal/play',
        'http://127.0.0.1/play',
        'http://127.8.9.10/play',
        'http://0.0.0.0/play',
        'http://10.1.2.3/play',
        'http://172.16.0.1/play',
        'http://172.31.255.254/play',
        'http://192.168.1.50/play',
        'http://169.254.169.254/latest/meta-data',
        'http://metadata.google.internal/computeMetadata/v1',
        'http://[::1]:3000/play',
        'http://[fe80::1]/play',
        'http://[fc00::1]/play',
        'http://[fd12:3456::1]/play',
        'http://[::ffff:127.0.0.1]/play',
    ];
    for (const target of blocked) {
        const out = inspectTarget(target);
        assert.equal(out?.code, 'BLOCKED_URL', `expected ${target} blocked`);
    }
});

test('isPrivateHostname handles edge formats', () => {
    assert.equal(isPrivateHostname('192.168.0.1'), true);
    assert.equal(isPrivateHostname('192.169.0.1'), false);
    assert.equal(isPrivateHostname('11.0.0.1'), false);
    assert.equal(isPrivateHostname('169.253.0.1'), false);
    assert.equal(isPrivateHostname('example.com'), false);
    assert.equal(isPrivateHostname(''), false);
});

test('validation error bodies include the legacy error key', () => {
    const out = run(validateParams(schemas.params.guildId, 'INVALID_SNOWFLAKE'), { params: { guildId: 'x' } });
    assert.equal(out.res.body.error, out.res.body.detail);
    assert.equal(out.res.body.status, 400);
});
