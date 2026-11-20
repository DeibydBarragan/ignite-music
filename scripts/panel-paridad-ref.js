'use strict';

const fs = require('node:fs');
const path = require('node:path');
const views = require('../src/ui/views');
const i18n = require('../src/core/i18n');
const { makeSong } = require('../test/helpers');

const targetDir = path.join(__dirname, '..', 'rust', 'crates', 'bot', 'tests', 'fixtures');
const targetFile = path.join(targetDir, 'panel-node.json');

const tr = (k, v) => i18n.t('en', k, v);

const baseState = (overrides = {}) => {
    const songs = Array.from({ length: 30 }, (_, i) => makeSong(i + 1));
    songs[0].member = undefined;
    return {
        song: songs[0],
        paused: false,
        currentTime: 42,
        volume: 80,
        repeatMode: 0,
        autoplay: false,
        filter: null,
        upcoming: songs.slice(1),
        upcomingDuration: 5000,
        ...overrides,
    };
};

const rawCases = {
    reproduciendo: views.playerPanel(tr, baseState()),
    en_pausa_bucle_cola_autoplay: views.playerPanel(tr, baseState({ paused: true, repeatMode: 2, autoplay: true })),
    en_directo: views.playerPanel(tr, baseState({ song: makeSong(98, { isLive: true }) })),
    duracion_cero: views.playerPanel(tr, baseState({ song: makeSong(96, { duration: 0 }) })),
    nombre_500_caracteres: views.playerPanel(tr, baseState({ song: makeSong(99, { name: 'x'.repeat(500), uploader: { name: 'y'.repeat(300) } }) })),
    sin_proximas: views.playerPanel(tr, baseState({ upcoming: [], upcomingDuration: 0 })),
    con_filtro: views.playerPanel(tr, baseState({ filter: 'nightcore' })),
};

const serialized = {};
for (const [key, payload] of Object.entries(rawCases)) {
    serialized[key] = {
        flags: payload.flags,
        components: payload.components.map((c) => c.toJSON()),
    };
}

fs.mkdirSync(targetDir, { recursive: true });
fs.writeFileSync(targetFile, JSON.stringify(serialized, null, 2), 'utf8');
console.log(`Generated ${targetFile} (${Object.keys(serialized).length} cases)`);
