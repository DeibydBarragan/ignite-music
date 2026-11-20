'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { problem } = require('../src/api/contracts');

const targetPath = path.join(
    __dirname,
    '..',
    'rust',
    'crates',
    'bot',
    'tests',
    'fixtures',
    'rutas-node.json',
);

const fixtures = {
    player_state_empty: {
        hasQueue: false,
        isPlaying: false,
        isPaused: false,
        currentTime: 0,
        duration: 0,
        volume: 80,
        repeatMode: 'off',
        autoplay: false,
        filter: null,
        voiceChannel: null,
        currentSong: null,
        queue: [],
        disconnectReason: null,
    },
    control_success: {
        success: true,
        message: 'Música pausada',
    },
    dev_mode_get: {
        success: true,
        devMode: false,
        userId: '123456789012345678',
    },
    dev_mode_post: {
        success: true,
        devMode: true,
        userId: '123456789012345678',
    },
    search_empty: {
        query: 'lofi beats',
        results: [],
    },
    suggestions_empty: {
        query: 'lofi',
        suggestions: [],
    },
    voice_settings_default: {
        enabled: false,
        wake_word: 'ignite',
        language: 'es',
        tts_feedback_enabled: false,
        tts_voice: 'es-MX-DaliaNeural',
        tts_pitch: 1.0,
        tts_rate: 1.0,
        tts_templates: {
            play: 'Reproduciendo {query}',
        },
        bindings: [],
    },
    blocked_url_problem: problem(
        400,
        'BLOCKED_URL',
        'URLs pointing to local or private networks are not allowed.',
    ),
    invalid_snowflake_problem: problem(
        400,
        'INVALID_SNOWFLAKE',
        'Guild ID must be a valid Discord snowflake.',
    ),
};

fs.mkdirSync(path.dirname(targetPath), { recursive: true });
fs.writeFileSync(targetPath, JSON.stringify(fixtures, null, 2) + '\n');
console.log(`Generated routes parity reference fixtures: ${targetPath}`);
