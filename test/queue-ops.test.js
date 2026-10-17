'use strict';

require('./helpers');
const test = require('node:test');
const assert = require('node:assert/strict');
const { ids } = require('./helpers');
const queueOps = require('../src/api/v1/ops/queue');

function fakeSong(n, extra = {}) {
    return {
        id: `vid${n}`,
        name: `Song ${n}`,
        url: `https://www.youtube.com/watch?v=vid${n}`,
        duration: 100 + n,
        thumbnail: `https://i.ytimg.com/vi/vid${n}/hq.jpg`,
        uploader: { name: `Artist ${n}` },
        ...extra,
    };
}

function fakeMusic(songs) {
    const queue = { id: ids.guild, songs };
    const events = { changed: 0, locked: 0 };
    const guild = { id: ids.guild, members: { me: {}, cache: new Map() } };
    const music = {
        distube: { getQueue: () => queue },
        client: { guilds: { cache: new Map([[ids.guild, guild]]) } },
        translator: () => (key) => key,
        withPlayLock: (guildId, task) => {
            events.locked++;
            return task();
        },
        changed: () => { events.changed++; },
        sessions: {
            serializeSong: (song) => ({ plugin: 'youtube', url: song.url, name: song.name, id: song.id, duration: song.duration, source: 'youtube', uploader: song.uploader }),
            buildSongs: (data) => data.songs.map((item) => fakeSong(item.id.replace('vid', ''), { url: item.url })),
        },
    };
    return { music, queue, events };
}

function isApiError(status, code) {
    return (error) => {
        assert.equal(error.name, 'ApiError');
        assert.equal(error.status, status);
        assert.equal(error.code, code);
        return true;
    };
}

test('removeIndex splices under the play lock and serializes the removed song', async () => {
    const { music, queue, events } = fakeMusic([fakeSong(1), fakeSong(2), fakeSong(3)]);
    const result = await queueOps.removeIndex(music, { guildId: ids.guild, index: 2 });

    assert.equal(result.success, true);
    assert.equal(result.removed.title, 'Song 3');
    assert.deepEqual(queue.songs.map((song) => song.name), ['Song 1', 'Song 2']);
    assert.equal(events.locked, 1);
    assert.equal(events.changed, 1);
});

test('removeIndex refuses the current song and out-of-range indexes', async () => {
    const { music } = fakeMusic([fakeSong(1), fakeSong(2)]);
    await assert.rejects(queueOps.removeIndex(music, { guildId: ids.guild, index: 0 }), isApiError(400, 'INVALID_INDEX'));
    await assert.rejects(queueOps.removeIndex(music, { guildId: ids.guild, index: 9 }), isApiError(400, 'INVALID_INDEX'));
});

test('removeTrack finds by trackId and never removes the current song', async () => {
    const current = fakeSong(1, { trackId: 'current-track-1' });
    const upcoming = fakeSong(2, { trackId: 'kZ9_x-12Ab' });
    const { music, queue } = fakeMusic([current, upcoming]);

    const result = await queueOps.removeTrack(music, { guildId: ids.guild, trackId: 'kZ9_x-12Ab' });
    assert.equal(result.success, true);
    assert.equal(result.removed.id, 'vid2');
    assert.equal(queue.songs.length, 1);

    await assert.rejects(
        queueOps.removeTrack(music, { guildId: ids.guild, trackId: 'current-track-1' }),
        isApiError(404, 'TRACK_NOT_FOUND'),
    );
    await assert.rejects(
        queueOps.removeTrack(music, { guildId: ids.guild, trackId: 'nope-nope-nope' }),
        isApiError(404, 'TRACK_NOT_FOUND'),
    );
});

test('order moves a trackId to a new position', async () => {
    const a = fakeSong(2, { trackId: 'track-aaaaaaa' });
    const b = fakeSong(3, { trackId: 'track-bbbbbbb' });
    const c = fakeSong(4, { trackId: 'track-ccccccc' });
    const { music, queue } = fakeMusic([fakeSong(1), a, b, c]);

    const result = await queueOps.order(music, { guildId: ids.guild, trackId: 'track-ccccccc', to: 1 });
    assert.equal(result.success, true);
    assert.deepEqual(queue.songs.map((song) => song.id), ['vid1', 'vid4', 'vid2', 'vid3']);
});

test('clear snapshots metadata and undo rebuilds the songs', async () => {
    const { music, queue } = fakeMusic([fakeSong(1), fakeSong(2), fakeSong(3)]);
    const cleared = await queueOps.clear(music, { guildId: ids.guild });
    assert.deepEqual(cleared, { success: true, cleared: 2 });
    assert.deepEqual(queue.songs.map((song) => song.id), ['vid1']);

    const undone = await queueOps.undo(music, { guildId: ids.guild });
    assert.equal(undone.success, true);
    assert.equal(undone.restored, 2);
    assert.deepEqual(queue.songs.map((song) => song.id), ['vid1', 'vid2', 'vid3']);

    await assert.rejects(queueOps.undo(music, { guildId: ids.guild }), isApiError(400, 'UNDO_EMPTY'));
});

test('queue mutations without an active queue report NO_QUEUE', async () => {
    const { music } = fakeMusic([]);
    music.distube.getQueue = () => undefined;
    await assert.rejects(queueOps.clear(music, { guildId: ids.guild }), isApiError(404, 'NO_QUEUE'));
    await assert.rejects(queueOps.move(music, { guildId: ids.guild, from: 1, to: 2 }), isApiError(404, 'NO_QUEUE'));
});

test('undo expires after the 60s undo TTL', async (t) => {
    t.mock.timers.enable({ apis: ['Date'] });
    const { music, queue } = fakeMusic([fakeSong(1), fakeSong(2), fakeSong(3)]);

    await queueOps.clear(music, { guildId: ids.guild });
    t.mock.timers.tick(60_001);

    await assert.rejects(queueOps.undo(music, { guildId: ids.guild }), isApiError(400, 'UNDO_EXPIRED'));
    assert.deepEqual(queue.songs.map((song) => song.id), ['vid1'], 'expired undo leaves the queue untouched');
});

test('concurrent removeTrack calls serialize through the play lock', async () => {
    const { music, queue } = fakeMusic([
        fakeSong(1),
        fakeSong(2, { trackId: 'track-aaaaaaa' }),
        fakeSong(3, { trackId: 'track-bbbbbbb' }),
        fakeSong(4),
    ]);

    // Real manager.withPlayLock serializes per guild; simulate that chain and
    // fire both API calls without awaiting the first one.
    let chain = Promise.resolve();
    music.withPlayLock = (guildId, task) => {
        const run = chain.then(() => task());
        chain = run.catch(() => undefined);
        return run;
    };

    const [first, second] = await Promise.all([
        queueOps.removeTrack(music, { guildId: ids.guild, trackId: 'track-aaaaaaa' }),
        queueOps.removeTrack(music, { guildId: ids.guild, trackId: 'track-bbbbbbb' }),
    ]);

    assert.equal(first.success, true);
    assert.equal(second.success, true);
    assert.deepEqual(queue.songs.map((song) => song.id), ['vid1', 'vid4']);
    await assert.rejects(
        queueOps.removeTrack(music, { guildId: ids.guild, trackId: 'track-aaaaaaa' }),
        isApiError(404, 'TRACK_NOT_FOUND'),
    );
});
