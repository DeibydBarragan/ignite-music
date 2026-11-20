'use strict';

require('./helpers');
const test = require('node:test');
const assert = require('node:assert/strict');
const player = require('../src/api/v1/ops/player');
const { playErrors, playSuccess } = require('./fixtures/api-contract/discovery-play.json');

test('play selects the caller channel and preserves the resolved song response', async () => {
    const guildId = '123456789012345678';
    const userId = '345678901234567890';
    const general = { id: '111111111111111111', name: 'General' };
    const stage = { id: '222222222222222222', name: 'Stage' };
    const member = { id: userId, displayName: 'Listener', user: { id: userId, username: 'listener' }, voice: { channel: null } };
    const guild = { id: guildId, members: { me: { voice: { channel: general } }, cache: new Map([[userId, member]]) } };
    const query = 'https://www.youtube.com/watch?v=dQw4w9WgXcQ';
    let calls = 0;
    const music = {
        client: { guilds: { cache: new Map([[guildId, guild]]) } },
        distube: { getQueue: () => null },
        play: async ({ voiceChannel, member: requester, query: requestedQuery }) => {
            calls++;
            assert.equal(voiceChannel, general);
            assert.equal(requester, member);
            assert.equal(requestedQuery, query);
            return { started: true, position: 0, resolved: {
                id: 'dQw4w9WgXcQ', name: 'Never Gonna Give You Up', uploader: { name: 'Rick Astley' },
                thumbnail: 'https://example.com/thumb.jpg', duration: 213, source: 'youtube', url: query, member,
            } };
        },
    };
    const input = { guildId, userId, query };
    for (const [channel, expected] of [[null, playErrors.outside], [stage, playErrors.different]]) {
        member.voice.channel = channel;
        await assert.rejects(player.play(music, input), (error) => {
            assert.deepEqual({ status: error.status, code: error.code, detail: error.message }, expected);
            return true;
        });
    }
    assert.equal(calls, 0);
    member.voice.channel = general;
    const result = await player.play(music, { ...input, voiceChannelId: stage.id });
    assert.match(result.song.trackId, /^[A-Za-z0-9_-]{8,64}$/);
    delete result.song.trackId;
    assert.deepEqual(result, playSuccess);
});

test('active player state matches the shared Rust/TypeScript contract fixture', () => {
    const expected = JSON.parse(require('node:fs').readFileSync(
        require('node:path').join(__dirname, 'fixtures/api-contract/player-active.json'), 'utf8',
    ));
    const queue = {
        songs: [{
            id: 'song-id', name: 'Song', uploader: { name: 'Artist' }, duration: 120,
            formattedDuration: '2:00', source: 'youtube', url: 'https://youtube.com/watch?v=song-id',
            trackId: 'track-contract-1',
        }],
        voiceChannel: expected.voiceChannel,
        paused: false,
        playing: true,
        repeatMode: require('distube').RepeatMode.SONG,
        volume: 65,
        autoplay: true,
        currentTime: 42,
        filters: { names: ['bassboost'] },
    };
    const result = player.state({ distube: { getQueue: () => queue }, state: () => ({}) }, { guildId: '123456789012345678' });
    assert.deepEqual(result, expected);
});

test('player control validation errors match shared Rust fixtures', async () => {
    const fs = require('node:fs');
    const path = require('node:path');
    const fixture = (name) => JSON.parse(fs.readFileSync(path.join(__dirname, `fixtures/api-contract/${name}.json`), 'utf8'));
    const noQueueMusic = { distube: { getQueue: () => null } };
    for (const [action, expected] of [['dance', fixture('control-invalid-action')], ['pause', fixture('control-no-queue')]]) {
        await assert.rejects(player.control(noQueueMusic, { guildId: '123456789012345678', action }), (error) => {
            assert.deepEqual({ status: error.status, code: error.code }, expected);
            return true;
        });
    }
});
