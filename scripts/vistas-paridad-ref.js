'use strict';

process.env.SUPPORT_SERVER = 'https://discord.gg/ignite';
process.env.WEBSITE = 'https://ignite.bot';
process.env.INVITE = 'https://discord.com/oauth2/authorize?client_id=123';

const fs = require('node:fs');
const path = require('node:path');
const { makeSong, fakeMember } = require('../test/helpers');
const i18n = require('../src/core/i18n');
const views = require('../src/ui/views');

const trEn = (key, vars) => i18n.t('en', key, vars);

function toJson(payload) {
    return {
        flags: payload.flags,
        components: payload.components.map((c) => c.toJSON()),
    };
}

const songs45 = Array.from({ length: 45 }, (_, i) => {
    const s = makeSong(i + 1);
    if (i === 0) s.member = fakeMember('111111111111111111', false);
    if (i === 1) s.member = fakeMember('222222222222222222', true);
    return s;
});

const lyricsLong = Array.from({ length: 200 }, (_, i) => `Line ${i} [chorus] *x*`).join('\n') + '\n\n' + 'z'.repeat(7000);

const searchResults = Array.from({ length: 10 }, (_, i) => makeSong(i + 1, { name: `Search Result ${i + 1}` }));

const helpCommands = Array.from({ length: 22 }, (_, i) => ({
    name: `cmd${i + 1}`,
    id: `12345678901234567${i}`,
    description: `Description for cmd${i + 1}`,
}));

const playlists15 = Array.from({ length: 15 }, (_, i) => ({
    name: `Playlist ${i + 1}`,
    url: `https://youtube.com/playlist?list=PL${i + 1}`,
    source: i % 2 === 0 ? 'youtube' : 'spotify',
    tracks: Array.from({ length: i + 2 }, () => ({ duration: 180 })),
}));

const fixture = {
    queue_page_0: toJson(views.queueView(trEn, { current: songs45[0], upcoming: songs45.slice(1), page: 0 })),
    queue_page_2: toJson(views.queueView(trEn, { current: songs45[0], upcoming: songs45.slice(1), page: 2 })),
    queue_page_last: toJson(views.queueView(trEn, { current: songs45[0], upcoming: songs45.slice(1), page: 99 })),
    queue_empty: toJson(views.queueView(trEn, { current: songs45[0], upcoming: [], page: 0 })),
    lyrics_paginated: toJson(views.lyricsView(trEn, { title: 'Song Title', artist: 'Artist Name', lyrics: lyricsLong, source: 'LRCLIB', token: 'deadbeef1234', page: 1 })),
    lyrics_short: toJson(views.lyricsView(trEn, { title: 'Short Song', lyrics: 'Line 1\nLine 2', source: 'LRCLIB', page: 0 })),
    search: toJson(views.searchView(trEn, { query: 'rock music', results: searchResults, token: 'deadbeef1234' })),
    help: toJson(views.helpView(trEn, {
        commands: helpCommands,
        stats: { servers: 10, active: 3, uptime: '1d 2h 30m', ping: 42 },
    })),
    language: toJson(views.languageView(trEn, { languages: i18n.list(), current: 'es' })),
    playlists_with_items: toJson(views.playlistsView(trEn, { playlists: playlists15, total: 15, page: 0, filter: 'rock' })),
    playlists_empty: toJson(views.playlistsView(trEn, { playlists: [], total: 0, page: 0, filter: null })),
    ended_panel: toJson(views.endedPanel(trEn, { title: 'Done', description: 'Playback finished', song: makeSong(1) })),
};

const outDir = path.join(__dirname, '../rust/crates/bot/tests/fixtures');
fs.mkdirSync(outDir, { recursive: true });
fs.writeFileSync(path.join(outDir, 'vistas-node.json'), JSON.stringify(fixture, null, 2), 'utf-8');
console.log('Generated vistas-node.json successfully');
