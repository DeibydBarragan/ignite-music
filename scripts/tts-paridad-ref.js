'use strict';

const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');

// Use a dummy secret for the reference: config.voice.soundApiSecret is overridden by env.
process.env.IGNITE_SOUND_API_SECRET = 'test-secret-value-do-not-use';
const {
    DEFAULT_TTS_TEMPLATES,
    interpolateTemplate,
    resolveFeedbackMessage,
    sendTtsFeedback,
} = require('../src/voice/tts-feedback');

const DATA_DIR = path.join(__dirname, '..', 'rust', 'crates', 'voice', 'tests', 'data');
const OUT_FILE = path.join(DATA_DIR, 'tts-casos.json');

async function captureRequest(fn) {
    let captured = null;
    const server = http.createServer((req, res) => {
        let body = '';
        req.on('data', (chunk) => {
            body += chunk;
        });
        req.on('end', () => {
            captured = {
                method: req.method,
                url: req.url,
                headers: {
                    authorization: req.headers.authorization || null,
                    'x-user-discord-id': req.headers['x-user-discord-id'] || null,
                    'content-type': req.headers['content-type'] || null,
                },
                body: JSON.parse(body),
            };
            res.writeHead(200, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify({ ok: true }));
        });
    });

    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    const port = server.address().port;
    const apiUrl = `http://127.0.0.1:${port}`;

    try {
        await fn(apiUrl);
    } finally {
        await new Promise((resolve) => server.close(resolve));
    }

    return captured;
}

async function main() {
    fs.mkdirSync(DATA_DIR, { recursive: true });

    // 1. Template resolutions
    const templateCases = [];
    const commands = Object.keys(DEFAULT_TTS_TEMPLATES);

    for (const cmd of commands) {
        templateCases.push({
            commandId: cmd,
            params: { query: 'Starboy' },
            resolved: resolveFeedbackMessage(cmd, { query: 'Starboy' }),
        });
        templateCases.push({
            commandId: cmd,
            params: {},
            resolved: resolveFeedbackMessage(cmd, {}),
        });
    }

    // Custom template override
    templateCases.push({
        commandId: 'play',
        params: { query: 'Starboy' },
        customTemplates: { play: 'Playing {query} now' },
        resolved: resolveFeedbackMessage('play', { query: 'Starboy' }, { play: 'Playing {query} now' }),
    });

    // Unknown template
    templateCases.push({
        commandId: 'non_existent_command',
        params: { query: 'Test' },
        resolved: resolveFeedbackMessage('non_existent_command', { query: 'Test' }),
    });

    // Interpolation direct checks
    const interpolationCases = [
        { template: 'Reproduciendo {query}', params: { query: 'Starboy' }, result: interpolateTemplate('Reproduciendo {query}', { query: 'Starboy' }) },
        { template: 'Reproduciendo {query}', params: {}, result: interpolateTemplate('Reproduciendo {query}', {}) },
        { template: 'Hola {user}, tu consulta es {query}', params: { user: 'Juan' }, result: interpolateTemplate('Hola {user}, tu consulta es {query}', { user: 'Juan' }) },
        { template: 'Sin tokens', params: {}, result: interpolateTemplate('Sin tokens', {}) },
    ];

    // 2. HTTP Requests captured from sendTtsFeedback
    const httpCases = [];

    // Full options case
    const cap1 = await captureRequest(async (apiUrl) => {
        await sendTtsFeedback({
            guildId: 'guild-111',
            channelId: 'channel-222',
            userId: 'user-333',
            commandId: 'play',
            params: { query: 'Bohemian Rhapsody' },
        }, {
            enabled: true,
            apiUrl,
            apiSecret: 'sec-secret-123',
            voice: 'es-MX-DaliaNeural',
            rate: 1.25,
            pitch: 0.9,
        });
    });
    httpCases.push({ label: 'full_options', captured: cap1 });

    // Minimal options case
    const cap2 = await captureRequest(async (apiUrl) => {
        await sendTtsFeedback({
            guildId: 'guild-999',
            channelId: 'channel-888',
            commandId: 'pause',
        }, {
            enabled: true,
            apiUrl,
        });
    });
    httpCases.push({ label: 'minimal_options', captured: cap2 });

    const output = {
        templateCases,
        interpolationCases,
        httpCases,
    };

    fs.writeFileSync(OUT_FILE, JSON.stringify(output, null, 2) + '\n', 'utf8');
}

main();
