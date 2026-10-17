'use strict';

require('./helpers');
const test = require('node:test');
const assert = require('node:assert/strict');
const { ids, makeSong } = require('./helpers');
const {
    ApiError,
    serializeTrack,
    ensureTrackId,
    formatDuration,
    getAvatarUrl,
    problem,
    sendProblem,
    sendError,
    sendOpResult,
    FALLBACK_COVER,
} = require('../src/api/contracts');

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

test('serializeTrack keeps the frontend Song contract shape', () => {
    const song = makeSong(1);
    const out = serializeTrack(song);

    assert.equal(out.id, song.id);
    assert.equal(out.title, song.name);
    assert.equal(out.artist, song.uploader.name);
    assert.equal(out.albumArt, song.thumbnail);
    assert.equal(out.duration, song.duration);
    assert.equal(out.source, 'youtube');
    assert.equal(out.url, song.url);
    assert.equal(out.category, 'hits');
    assert.equal(typeof out.trackId, 'string');
    assert.equal(typeof out.isLyric, 'boolean');
    assert.equal(typeof out.isAutoplay, 'boolean');
});

test('trackId is stable across serializations of the same Song', () => {
    const song = makeSong(2);
    const first = serializeTrack(song);
    const second = serializeTrack(song);
    assert.equal(first.trackId, second.trackId);
    assert.notEqual(first.trackId, serializeTrack(makeSong(3)).trackId);
});

test('unknown sources are clamped to the dashboard enum', () => {
    const song = makeSong(4, { source: 'generic' });
    assert.equal(serializeTrack(song).source, 'youtube');
    const spotify = makeSong(5, { source: 'spotify' });
    assert.equal(serializeTrack(spotify).source, 'spotify');
});

test('serializeTrack extended:false omits queue-only fields', () => {
    const song = makeSong(6);
    const snapshot = serializeTrack(song, { extended: false });
    assert.equal(snapshot.trackId, undefined);
    assert.equal(snapshot.requestedBy, undefined);
    assert.equal(snapshot.isAutoplay, undefined);
    assert.equal(snapshot.albumArt, song.thumbnail);
});

test('serializeTrack falls back to cover art and labels', () => {
    const out = serializeTrack({ url: 'https://x/y', source: 'youtube' });
    assert.equal(out.albumArt, FALLBACK_COVER);
    assert.equal(out.title, 'Unknown Title');
    assert.equal(out.artist, 'Unknown Artist');
    assert.equal(out.formattedDuration, '00:00');
});

test('serializeTrack never leaks non-spotify sources or crashes without url', () => {
    assert.equal(serializeTrack(null), null);
    const out = serializeTrack(makeSong(7));
    assert.equal(out.isLive, false);
});

test('ensureTrackId assigns once and reuses', () => {
    const song = makeSong(8);
    const id = ensureTrackId(song);
    assert.equal(song.trackId, id);
    assert.equal(ensureTrackId(song), id);
});

test('formatDuration renders mm:ss', () => {
    assert.equal(formatDuration(0), '0:00');
    assert.equal(formatDuration(61), '1:01');
    assert.equal(formatDuration(125), '2:05');
    assert.equal(formatDuration(undefined), '0:00');
});

test('getAvatarUrl prefers displayAvatarURL and tolerates plain strings', () => {
    assert.equal(getAvatarUrl({ displayAvatarURL: () => 'http://a/b.png' }), 'http://a/b.png');
    assert.equal(getAvatarUrl({ avatar: 'https://cdn/x.png' }), 'https://cdn/x.png');
    assert.equal(getAvatarUrl({ displayAvatarURL: () => { throw new Error('boom'); }, avatar: 'https://cdn/y.png' }), 'https://cdn/y.png');
    assert.equal(getAvatarUrl(null), null);
});

test('problem documents follow RFC 7807 with the legacy error key', () => {
    const body = problem(400, 'INVALID_SNOWFLAKE', 'must be a Discord snowflake');
    assert.equal(body.status, 400);
    assert.equal(body.code, 'INVALID_SNOWFLAKE');
    assert.equal(body.type, 'https://ignite.bot/errors/invalid-snowflake');
    assert.equal(body.title, 'Invalid Request Parameter');
    assert.equal(body.detail, body.error);
    assert.ok(!Number.isNaN(Date.parse(body.timestamp)));
});

test('sendProblem writes application/problem+json', () => {
    const res = fakeRes();
    sendProblem(res, 404, 'NOT_FOUND', 'Guild not found.');
    assert.equal(res.code, 404);
    assert.equal(res.typeHeader, 'application/problem+json');
    assert.equal(res.body.code, 'NOT_FOUND');
    assert.equal(res.body.error, 'Guild not found.');
});

test('sendError maps ApiError fields and anonymizes unknown errors', () => {
    const res = fakeRes();
    sendError(res, new ApiError(409, 'ALREADY_PLAYING', 'Queue busy.'));
    assert.equal(res.code, 409);
    assert.equal(res.body.code, 'ALREADY_PLAYING');
    assert.equal(res.body.title, 'Conflict');

    const res2 = fakeRes();
    sendError(res2, new Error('secret internal detail'));
    assert.equal(res2.code, 500);
    assert.equal(res2.body.code, 'INTERNAL_ERROR');
    assert.equal(res2.body.detail, 'secret internal detail');
});

test('sendOpResult maps proxy results to JSON or problem documents', () => {
    const ok = fakeRes();
    sendOpResult(ok, { ok: true, data: { success: true, started: false } });
    assert.equal(ok.code, null, 'success responses keep their status');
    assert.deepEqual(ok.body, { success: true, started: false });

    const transported = fakeRes();
    sendOpResult(transported, { ok: false, code: 'NO_QUEUE', status: 404, message: 'No queue found' });
    assert.equal(transported.code, 404);
    assert.equal(transported.body.code, 'NO_QUEUE');

    const mapped = fakeRes();
    sendOpResult(mapped, { ok: false, code: 'guild_not_found', message: 'The bot is not in this guild' });
    assert.equal(mapped.code, 404);

    const fallback = fakeRes();
    sendOpResult(fallback, { ok: false, message: 'weird failure' });
    assert.equal(fallback.code, 500);
    assert.equal(fallback.body.code, 'op_failed');
});

test('ApiError derives title and type from status/code', () => {
    const err = new ApiError(503, 'shard_timeout', 'Shard did not answer.');
    assert.equal(err.name, 'ApiError');
    assert.equal(err.status, 503);
    assert.equal(err.code, 'shard_timeout');
    assert.equal(err.title, 'Service Unavailable');
    assert.equal(err.type, 'https://ignite.bot/errors/shard-timeout');
    assert.equal(err.message, 'Shard did not answer.');
    assert.ok(err instanceof Error);
});

test('serializeTrack surfaces the requesting member', () => {
    // DisTube's `member` setter only accepts real GuildMember instances, so
    // unit tests go through the metadata.requesterUser fallback.
    const song = makeSong(9);
    song.metadata.requesterUser = { id: ids.user, username: 'tester' };
    const out = serializeTrack(song);
    assert.equal(out.requestedBy.id, ids.user);
    assert.equal(out.requestedBy.username, 'tester');
    assert.equal(out.isAutoplay, false);
});

test('serializeTrack marks bot requesters as autoplay', () => {
    const song = makeSong(10);
    song.metadata.requesterUser = { id: ids.bot, bot: true, username: 'Bot' };
    const out = serializeTrack(song);
    assert.equal(out.isAutoplay, true);
    assert.equal(out.requestedBy.username, 'Bot');
});
