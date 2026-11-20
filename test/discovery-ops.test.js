'use strict';

require('./helpers');
const test = require('node:test');
const assert = require('node:assert/strict');
const discovery = require('../src/api/v1/ops/discovery');
const { Collection } = require('discord.js');
const { channels: voiceFixture } = require('./fixtures/api-contract/discovery-play.json');

test('channels lists occupied and empty voice channels with only human members', async () => {
    const botId = '333333333333333333';
    const otherBot = '444444444444444444';
    const userId = '987654321098765432';
    const stage = { id: '222222222222222222', name: 'Stage', bitrate: 96000, position: 0, isVoiceBased: () => true, members: new Collection() };
    const general = { id: '111111111111111111', name: 'General', bitrate: 64000, userLimit: 26, position: 1, isVoiceBased: () => true,
        members: new Collection([
            [botId, { id: botId, user: { bot: true } }],
            [userId, { id: userId, user: { username: 'listener', bot: false,
                displayAvatarURL: () => 'https://cdn.discordapp.com/embed/avatars/3.png' } }],
            [otherBot, { id: otherBot, user: { bot: true } }],
        ]) };
    const channels = new Collection([[general.id, general], ['text', { isVoiceBased: () => false }], [stage.id, stage]]);
    const guildId = '123456789012345678';
    const guild = { channels: { cache: channels, fetch: async () => channels } };
    const music = { client: { user: { id: botId }, guilds: { cache: new Map([[guildId, guild]]) } } };
    assert.deepEqual(await discovery.channels(music, { guildId }), voiceFixture);

    for (let i = 0; i < 25; i++) {
        const id = String(999999999999999999n - BigInt(i));
        general.members.set(id, { id, user: { username: 'listener', bot: false } });
    }
    const { channels: [, occupied] } = await discovery.channels(music, { guildId });
    assert.equal(occupied.userCount, 26);
    assert.equal(occupied.members.length, 25);
    assert.equal(occupied.members[0].id, userId);
});

test('search, playlist search and suggestions short-circuit invalid query shapes', async () => {
    let calls = 0;
    const music = {
        youtube: {
            search: async () => { calls++; throw new Error('must not search'); },
            searchPlaylists: async () => { calls++; throw new Error('must not search'); },
        },
    };
    const inputs = ['', 'a', 'x'.repeat(101), 'https://youtube.com/watch?v=video'];

    for (const raw of inputs) {
        const q = raw.trim();
        assert.deepEqual(await discovery.search(music, { q: raw }), { query: q, results: [] });
        assert.deepEqual(await discovery.searchPlaylists(music, { q: raw }), { query: q, results: [] });
        assert.deepEqual(await discovery.suggest(music, { q: raw }), { query: q, suggestions: [] });
    }
    assert.equal(calls, 0, 'invalid queries never invoke external search providers');
});

test('voice status only grants control to a developer or a user sharing the bot channel', async () => {
    const userId = '987654321098765432';
    const userChannel = { id: 'voice-1', name: 'General' };
    const makeMusic = (botChannel) => ({
        client: {
            guilds: {
                cache: new Map([['123456789012345678', {
                    members: {
                        me: { voice: { channel: botChannel, serverDeaf: false, deaf: false, selfDeaf: false } },
                        cache: new Map([[userId, { voice: { channel: userChannel } }]]),
                    },
                }]]),
            },
        },
    });

    const outsideChannel = await discovery.voiceStatus(makeMusic(null), {
        guildId: '123456789012345678', userId,
    });
    assert.equal(outsideChannel.canControl, false);

    const sameChannel = await discovery.voiceStatus(makeMusic(userChannel), {
        guildId: '123456789012345678', userId,
    });
    assert.equal(sameChannel.canControl, true);
});

test('empty discovery payloads and playlist resolution match shared API fixtures', async () => {
    const fixture = (name) => JSON.parse(require('node:fs').readFileSync(
        require('node:path').join(__dirname, 'fixtures/api-contract', name), 'utf8',
    ));
    const guildId = '123456789012345678';
    const music = {
        distube: { getQueue: () => undefined },
        client: { guilds: { cache: new Map([[guildId, { members: { me: {} }, channels: {
            cache: { filter: () => new Map() }, fetch: async () => [],
        } }]]) } },
    };
    const topSongsModule = require('../src/music/top-songs');
    const originalGetTopSongs = topSongsModule.getTopSongs;
    topSongsModule.getTopSongs = async () => [];
    try {
        assert.deepEqual(await discovery.channels(music, { guildId }), fixture('discovery-channels-empty.json'));
        assert.deepEqual(await discovery.recommendations(music, { guildId }), fixture('discovery-recommendations-empty.json'));
        assert.deepEqual(await discovery.topSongs(music, { guildId }), fixture('discovery-top-songs-empty.json'));
    } finally {
        topSongsModule.getTopSongs = originalGetTopSongs;
    }

    const playlists = require('../src/music/playlists');
    const originalResolve = playlists.resolvePlaylist;
    playlists.resolvePlaylist = async () => ({
        kind: 'playlist', name: 'Empty playlist', source: 'youtube', tracks: [], truncated: false,
    });
    try {
        assert.deepEqual(await discovery.resolvePlaylist(music, { guildId, url: 'https://youtube.com/playlist?list=PLtest' }),
            fixture('playlist-resolve-empty.json'));
    } finally {
        playlists.resolvePlaylist = originalResolve;
    }
});
