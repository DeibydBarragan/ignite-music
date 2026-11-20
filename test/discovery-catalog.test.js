'use strict';

// Install the test double before loading the Supabase client.
process.env.SUPABASE_URL = 'https://example.test';
process.env.SUPABASE_SERVICE_ROLE_KEY = 'offline-test-key';
require('./helpers');
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { Playlist } = require('distube');
const discovery = require('../src/api/v1/ops/discovery');
const { supabase } = require('../src/core/supabase');
const { serializePlaylist } = require('../src/api/contracts');
const { YouTubePlugin } = require('../src/music/plugins');
const fixture = require('./fixtures/api-contract/discovery-catalog.json');
const rows = require('../rust/crates/bot/tests/fixtures/supabase/top_songs_rpc.json');

const guildId = '123456789012345678';

test('ranking and fallback recommendations keep the same rank, count and date as Rust', async (t) => {
    t.mock.method(supabase, 'rpc', async () => ({ data: rows, error: null }));
    const music = { distube: { getQueue: () => null } };
    const top = await discovery.topSongs(music, { guildId, timeframe: 'all' });
    assert.deepEqual(top, fixture.topSongs);
    const recommended = await discovery.recommendations(music, { guildId });
    assert.deepEqual(recommended, { success: true, source: 'top', basedOn: null, songs: fixture.topSongs.songs });
});

test('playlist resolves at the exact limit without inventing queue-only fields', async (t) => {
    const url = 'https://media.example.test/album';
    const guild = { id: guildId, members: { me: {} } };
    const album = new Playlist({ source: 'generic', id: 'al', name: 'Album', url, songs: [
        { id: '1', name: 'a', url: 'https://x/1', source: 'generic' },
        { id: '2', name: 'b', url: 'https://x/2', source: 'generic' },
    ] });
    const music = {
        client: { guilds: { cache: new Map([[guildId, guild]]) } },
        distube: { handler: { resolve: async () => album } },
    };
    const playlists = require('../src/music/playlists');
    const resolvePlaylist = playlists.resolvePlaylist;
    t.mock.method(playlists, 'resolvePlaylist', (manager, server, input, options) => resolvePlaylist(
        manager, server, input, { ...options, maxSize: 2 },
    ));
    assert.deepEqual(await discovery.resolvePlaylist(music, { guildId, url }), fixture.playlist);
});

test('playlist search fallback preserves uploader and track count and honors the limit', async () => {
    const plugin = {
        directEntries: async () => null,
        run: async () => ({ entries: [null,
            { id: 'PL_FOUND', title: 'Found Playlist', uploader: 'Curator',
                thumbnail: 'https://thumb/1.jpg', playlist_count: 37 },
            { id: 'PL_SECOND', title: 'Second Playlist', n_entries: 2 },
        ] }),
    };
    const entries = await YouTubePlugin.prototype.searchPlaylists.call(plugin, 'test album', 1);
    assert.deepEqual(entries.map(serializePlaylist), [fixture.playlistSearch]);
});

test('contextual radio uses a cache and force resolves a new set', async () => {
    const song = { id: 'current', name: 'Current', url: 'https://example.test/current', trackId: 'current-1',
        member: { user: { id: '987654321098765432', username: 'Listener' } } };
    let calls = 0;
    const music = {
        distube: { getQueue: () => ({ songs: [song] }) },
        youtube: { getRelatedSongs: async () => [{ id: `related-${++calls}`, name: 'Related', url: 'https://example.test/related' }] },
    };
    const first = await discovery.recommendations(music, { guildId });
    assert.equal(first.source, 'radio');
    assert.equal(first.basedOn.isAutoplay, false);
    assert.equal(first.songs[0].id, 'related-1');
    assert.deepEqual(await discovery.recommendations(music, { guildId }), first);
    assert.equal((await discovery.recommendations(music, { guildId, force: true })).songs[0].id, 'related-2');
    assert.equal(calls, 2);
});
