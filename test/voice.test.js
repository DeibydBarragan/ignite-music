'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const {
    downsample48kStereoTo16kMono,
    pcmToWav,
    slicePcmWav,
    transcribeAudio,
    GROQ_API_URL,
    VoiceReceiverManager,
} = require('../src/voice');

test('downsample48kStereoTo16kMono: downsamples 3:1 and averages stereo channels', () => {
    // 3 frames of 48kHz stereo: frame = 4 bytes (L: 2 bytes, R: 2 bytes)
    // 3 frames = 12 bytes -> produces 1 mono sample = 2 bytes
    const input = Buffer.alloc(12);
    // Frame 0: L = 1000, R = 3000 -> average = 2000
    input.writeInt16LE(1000, 0);
    input.writeInt16LE(3000, 2);
    // Frame 1: L = 500, R = 500
    input.writeInt16LE(500, 4);
    input.writeInt16LE(500, 6);
    // Frame 2: L = 200, R = 200
    input.writeInt16LE(200, 8);
    input.writeInt16LE(200, 10);

    const output = downsample48kStereoTo16kMono(input);
    assert.equal(output.length, 2);
    assert.equal(output.readInt16LE(0), 2000);

    // Empty or too small buffers return empty buffer
    assert.equal(downsample48kStereoTo16kMono(Buffer.alloc(8)).length, 0);
    assert.equal(downsample48kStereoTo16kMono(null).length, 0);
});

test('pcmToWav: generates a valid 44-byte RIFF/WAVE container', () => {
    const fakePcm = Buffer.alloc(1600); // 100ms of 16kHz 16-bit mono
    const wav = pcmToWav(fakePcm, 16000, 1, 16);

    assert.equal(wav.length, 44 + 1600);
    assert.equal(wav.subarray(0, 4).toString(), 'RIFF');
    assert.equal(wav.readUInt32LE(4), 36 + 1600); // ChunkSize
    assert.equal(wav.subarray(8, 12).toString(), 'WAVE');
    assert.equal(wav.subarray(12, 16).toString(), 'fmt ');
    assert.equal(wav.readUInt32LE(16), 16); // Subchunk1Size
    assert.equal(wav.readUInt16LE(20), 1); // PCM format
    assert.equal(wav.readUInt16LE(22), 1); // 1 channel (mono)
    assert.equal(wav.readUInt32LE(24), 16000); // 16kHz
    assert.equal(wav.readUInt32LE(28), 32000); // ByteRate (16000 * 1 * 2)
    assert.equal(wav.readUInt16LE(32), 2); // BlockAlign
    assert.equal(wav.readUInt16LE(34), 16); // BitsPerSample
    assert.equal(wav.subarray(36, 40).toString(), 'data');
    assert.equal(wav.readUInt32LE(40), 16000 ? 1600 : 0); // DataSize
});
test('slicePcmWav: trims audio before wake word with pre-roll and generates valid WAV', () => {
    // 2 seconds of 16kHz 16-bit mono = 64,000 bytes
    const pcm2s = Buffer.alloc(64000);
    // Wake word starts at 1,000 ms (1s)
    // Pre-roll = 250 ms -> effective start = 750 ms
    // 750 ms * 32 bytes/ms = 24,000 bytes offset
    // Remaining bytes: 64,000 - 24,000 = 40,000 bytes
    const sliced = slicePcmWav(pcm2s, 1000, 250);
    assert.equal(sliced.pcm16k.length, 40000);
    assert.equal(sliced.wavBuffer.length, 44 + 40000);
    assert.equal(sliced.wavBuffer.subarray(0, 4).toString(), 'RIFF');

    // Edge case 1: startMs <= 0 returns full audio
    const full = slicePcmWav(pcm2s, 0);
    assert.equal(full.pcm16k.length, 64000);

    // Edge case 2: startMs with pre-roll <= 0 starts at 0
    const early = slicePcmWav(pcm2s, 200, 250);
    assert.equal(early.pcm16k.length, 64000);

    // Edge case 3: over-slicing protection (wake word too close to end)
    const late = slicePcmWav(pcm2s, 1900, 250);
    assert.equal(late.pcm16k.length, 64000);
});


test('groqClient: transcribeAudio requires API key and handles empty audio', async () => {
    await assert.rejects(
        async () => transcribeAudio(Buffer.alloc(100), { apiKey: '' }),
        /Groq API Key is not configured/
    );

    const emptyResult = await transcribeAudio(Buffer.alloc(40), { apiKey: 'gsk_test' });
    assert.deepEqual(emptyResult, { text: '' });
});

test('groqClient: transcribeAudio sends multipart request and returns parsed text', async () => {
    const originalFetch = globalThis.fetch;
    let capturedUrl = null;
    let capturedAuth = null;

    globalThis.fetch = async (url, options) => {
        capturedUrl = url;
        capturedAuth = options.headers.Authorization;
        return {
            ok: true,
            json: async () => ({ text: 'ignite reproduce bad bunny' }),
        };
    };

    try {
        const fakeWav = pcmToWav(Buffer.alloc(100));
        const result = await transcribeAudio(fakeWav, {
            apiKey: 'gsk_mock_12345',
            language: 'es',
        });

        assert.equal(capturedUrl, GROQ_API_URL);
        assert.equal(capturedAuth, 'Bearer gsk_mock_12345');
        assert.equal(result.text, 'ignite reproduce bad bunny');
        assert.ok(typeof result.elapsedMs === 'number');
    } finally {
        globalThis.fetch = originalFetch;
    }
});

test('VoiceReceiverManager: attaches speaking listener and detaches cleanly', () => {
    const manager = new VoiceReceiverManager();
    const fakeSpeaking = new EventEmitter();
    const fakeReceiver = { speaking: fakeSpeaking, subscribe: () => new EventEmitter() };
    const fakeConnection = { receiver: fakeReceiver, joinConfig: { channelId: '123' } };

    // Attached with voice enabled
    manager.attach('guild-1', fakeConnection, { enabled: true, apiKey: 'gsk_test' });
    assert.equal(manager.listeners.has('guild-1'), true);
    assert.equal(fakeSpeaking.listenerCount('start'), 1);

    // Detached
    manager.detach('guild-1');
    assert.equal(manager.listeners.has('guild-1'), false);
    assert.equal(fakeSpeaking.listenerCount('start'), 0);
});

test('matcher: normalizes and matches default voice commands', () => {
    const { matchVoiceCommand, normalizeText } = require('../src/voice');

    assert.equal(normalizeText('¿¡Hola, Mundo...!?'), 'hola mundo');

    // Missing wake word
    const noWake = matchVoiceCommand('pon bohemian rhapsody', { wakeWord: 'ignite' });
    assert.equal(noWake.matched, false);
    assert.equal(noWake.reason, 'wake_word_missing');

    // Play command with query
    const playMatch = matchVoiceCommand('Ignite, reproduce Bohemian Rhapsody!', { wakeWord: 'ignite' });
    assert.equal(playMatch.matched, true);
    assert.equal(playMatch.commandId, 'play');
    assert.equal(playMatch.phrase, 'reproduce');
    assert.equal(playMatch.query, 'bohemian rhapsody');
    assert.equal(playMatch.missingQuery, false);

    // Command without argument (skip)
    const skipMatch = matchVoiceCommand('ignite siguiente', { wakeWord: 'ignite' });
    assert.equal(skipMatch.matched, true);
    assert.equal(skipMatch.commandId, 'skip');
    assert.equal(skipMatch.phrase, 'siguiente');

    // Command requiring argument but missing query
    const missingArg = matchVoiceCommand('ignite pon', { wakeWord: 'ignite' });
    assert.equal(missingArg.matched, true);
    assert.equal(missingArg.commandId, 'play');
    assert.equal(missingArg.missingQuery, true);

    // Custom wake word and custom phrase binding
    const customMatch = matchVoiceCommand('bot tocate Something About Us', {
        wakeWord: 'bot',
        customBindings: { play: ['tocate'] },
    });
    assert.equal(customMatch.matched, true);
    assert.equal(customMatch.commandId, 'play');
    assert.equal(customMatch.phrase, 'tocate');
    assert.equal(customMatch.query, 'something about us');

    // Speech with words preceding the wake word (e.g. conversational prelude)
    const preludeMatch1 = matchVoiceCommand('Creo que se hace un poco más rápido. Bot, pausa.', { wakeWord: 'bot' });
    assert.equal(preludeMatch1.matched, true);
    assert.equal(preludeMatch1.commandId, 'pause');

    const preludeMatch2 = matchVoiceCommand('No, no funcionó. Otra vez. Bot, pausa.', { wakeWord: 'bot' });
    assert.equal(preludeMatch2.matched, true);
    assert.equal(preludeMatch2.commandId, 'pause');

    // Conversational filler between wake word and command
    const fillerMatch = matchVoiceCommand('bot por favor puedes reproducir Starboy', { wakeWord: 'bot' });
    assert.equal(fillerMatch.matched, true);
    assert.equal(fillerMatch.commandId, 'play');
    assert.equal(fillerMatch.query, 'starboy');
});

test('ttsFeedback: interpolates templates and formats messages', () => {
    const { interpolateTemplate, resolveFeedbackMessage } = require('../src/voice');

    assert.equal(interpolateTemplate('Reproduciendo {query}', { query: 'Starboy' }), 'Reproduciendo Starboy');
    assert.equal(interpolateTemplate('Saltando canción', {}), 'Saltando canción');

    const defaultMsg = resolveFeedbackMessage('play', { query: 'In The End' });
    assert.equal(defaultMsg, 'Reproduciendo In The End');

    const customMsg = resolveFeedbackMessage('play', { query: 'Numb' }, { play: 'Poniendo el temazo {query}' });
    assert.equal(customMsg, 'Poniendo el temazo Numb');
});

test('ttsFeedback: sendTtsFeedback sends request to ignite-sound with userId header and body', async () => {
    const { sendTtsFeedback } = require('../src/voice');
    const originalFetch = globalThis.fetch;
    let postedUrl = null;
    let postedBody = null;
    let postedHeaders = null;

    globalThis.fetch = async (url, options) => {
        postedUrl = url;
        postedHeaders = options.headers;
        postedBody = JSON.parse(options.body);
        return { ok: true };
    };

    try {
        const sent = await sendTtsFeedback({
            guildId: '1011718919473610863',
            channelId: '987654321',
            userId: '112233445566778899',
            commandId: 'skip',
        }, {
            enabled: true,
            apiUrl: 'http://localhost:3002',
            apiSecret: 'secret_12345',
        });

        assert.equal(sent, true);
        assert.equal(postedUrl, 'http://localhost:3002/api/v1/tts/speak');
        assert.equal(postedHeaders.Authorization, 'Bearer secret_12345');
        assert.equal(postedHeaders['X-User-Discord-Id'], '112233445566778899');
        assert.equal(postedBody.guildId, '1011718919473610863');
        assert.equal(postedBody.userId, '112233445566778899');
        assert.equal(postedBody.text, 'Saltando canción');
    } finally {
        globalThis.fetch = originalFetch;
    }
});

test('VoiceReceiverManager: ignores speech from bots and self', async () => {
    const manager = new VoiceReceiverManager();
    const fakeSpeaking = new EventEmitter();
    let subscribeCalled = false;
    const fakeReceiver = {
        speaking: fakeSpeaking,
        subscribe: () => {
            subscribeCalled = true;
            return new EventEmitter();
        },
    };
    const fakeConnection = { receiver: fakeReceiver, joinConfig: { channelId: '123' } };

    const mockClient = {
        user: { id: 'bot-self-id' },
        guilds: {
            cache: {
                get: (_gid) => ({
                    members: {
                        cache: {
                            get: (uid) => uid === 'other-bot-id' ? { user: { bot: true } } : { user: { bot: false } },
                        },
                    },
                }),
            },
        },
    };

    manager.setClient(mockClient);
    manager.attach('guild-1', fakeConnection, { enabled: true, apiKey: 'gsk_test' });

    // 1. Emit self speaking -> should be ignored without subscribing
    fakeSpeaking.emit('start', 'bot-self-id');
    assert.equal(subscribeCalled, false);

    // 2. Emit other bot speaking -> should be ignored without subscribing
    fakeSpeaking.emit('start', 'other-bot-id');
    assert.equal(subscribeCalled, false);

    manager.detach('guild-1');
});

test('dispatcher: dispatches play and control commands to manager', async () => {
    const { dispatchVoiceCommand } = require('../src/voice');

    let playedQuery = null;
    let playedSkip = null;
    let playedPosition = null;
    let controlCalled = null;

    const mockManager = {
        client: {
            guilds: {
                cache: new Map([
                    ['g1', {
                        channels: { cache: new Map([['c1', { id: 'c1' }]]) },
                        members: { cache: new Map([['u1', { id: 'u1', voice: { channel: { id: 'c1' } } }]]) },
                    }],
                ]),
            },
        },
        getQueue: () => ({
            songs: [{ name: 'Track 1' }],
            paused: false,
            pause: async () => { controlCalled = 'pause'; },
        }),
        translator: () => (k) => k,
        play: async ({ query, skip, position }) => {
            playedQuery = query;
            playedSkip = skip;
            playedPosition = position;
            return { ok: true };
        },
    };

    // Test dispatch play (skip = true)
    const playResult = await dispatchVoiceCommand(mockManager, {
            guildId: 'g1',
            userId: 'u1',
            text: 'ignite reproduce starboy',
            channelId: 'c1',
        }, {
            settings: { wake_word: 'ignite', tts_feedback_enabled: false },
        });

        assert.equal(playResult.executed, true);
        assert.equal(playResult.commandId, 'play');
        assert.equal(playedQuery, 'starboy');
        assert.equal(playedSkip, true);

    // Test dispatch play_next (position = 1, skip = false)
    const nextResult = await dispatchVoiceCommand(mockManager, {
            guildId: 'g1',
            userId: 'u1',
            text: 'bot pon despues bohemian rhapsody',
            channelId: 'c1',
        }, {
            settings: { wake_word: 'bot', tts_feedback_enabled: false },
        });

        assert.equal(nextResult.executed, true);
        assert.equal(nextResult.commandId, 'play_next');
        assert.equal(playedQuery, 'bohemian rhapsody');
        assert.equal(playedSkip, false);
        assert.equal(playedPosition, 1);

        // Test dispatch pause
        const pauseResult = await dispatchVoiceCommand(mockManager, {
            guildId: 'g1',
            userId: 'u1',
            text: 'ignite pausa',
            channelId: 'c1',
        }, {
            settings: { wake_word: 'ignite', tts_feedback_enabled: false },
        });

        assert.equal(pauseResult.executed, true);
        assert.equal(pauseResult.commandId, 'pause');
        assert.equal(controlCalled, 'pause');
});

test('dispatcher: autoplay on/off sets an explicit state (idempotent)', async () => {
    const { dispatchVoiceCommand, matchVoiceCommand } = require('../src/voice');

    // Matcher: variants Whisper may write, plus the off/stop collision ("apaga")
    const cases = [
        ['ignite activa el autoplay', 'autoplay_on'],
        ['ignite enciende el modo auto play por favor', 'autoplay_on'],
        ['ignite activa la reproducción automática', 'autoplay_on'],
        ['ignite desactiva el autoplay', 'autoplay_off'],
        ['ignite apaga el autoplay', 'autoplay_off'],
        ['ignite apaga', 'stop'],
    ];
    for (const [text, expected] of cases) {
        const m = matchVoiceCommand(text, { wakeWord: 'ignite' });
        assert.equal(m.commandId, expected, text);
    }
    assert.equal(matchVoiceCommand('ignite activa el autoplay de la fiesta', { wakeWord: 'ignite' }).matched, false);

    let toggles = 0;
    let refreshed = 0;
    const queue = {
        autoplay: false,
        songs: [{ name: 'Track 1' }],
        toggleAutoplay() { toggles += 1; this.autoplay = !this.autoplay; return this.autoplay; },
    };
    const mockManager = {
        client: {
            guilds: {
                cache: new Map([['g1', {
                    channels: { cache: new Map() },
                    members: { cache: new Map([['u1', { id: 'u1', voice: { channel: { id: 'c1' } } }]]) },
                }]]),
            },
        },
        getQueue: () => queue,
        translator: () => (k) => k,
        refreshPanel: () => { refreshed += 1; },
    };
    const run = (text) => dispatchVoiceCommand(mockManager, { guildId: 'g1', userId: 'u1', text, channelId: 'c1' }, {
        settings: { wake_word: 'ignite', tts_feedback_enabled: false },
    });

    const on = await run('ignite activa el autoplay');
    assert.equal(on.executed, true);
    assert.equal(queue.autoplay, true);

    // Already on: saying "on" again must not turn it off
    await run('ignite activa el autoplay');
    assert.equal(queue.autoplay, true);
    assert.equal(toggles, 1);

    const off = await run('ignite desactiva el autoplay');
    assert.equal(off.executed, true);
    assert.equal(queue.autoplay, false);
    assert.equal(refreshed, 3);
});
