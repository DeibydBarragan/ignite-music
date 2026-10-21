'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const {
    VoskModelManager,
    ensureVoskBinary,
    ensureVoskModel,
    matchVoiceCommand,
    parseWakeWords,
} = require('../src/voice');

const FIXTURES_DIR = path.join(__dirname, 'fixtures');
const IGNITE_SAMPLE = path.join(FIXTURES_DIR, 'ignite-sample.wav');
const CASUAL_SAMPLE = path.join(FIXTURES_DIR, 'casual-speech-sample.wav');

test('Vosk Downloader: binary and model paths exist or resolve', async () => {
    const binPath = await ensureVoskBinary();
    assert.ok(binPath, 'ensureVoskBinary should return a valid binary path on win32 x64 or linux x64');
    assert.equal(fs.existsSync(binPath), true);

    const modelDir = await ensureVoskModel();
    assert.ok(modelDir, 'ensureVoskModel should return a valid model directory');
    assert.equal(fs.existsSync(modelDir), true);
    assert.equal(fs.existsSync(path.join(modelDir, 'am', 'final.mdl')), true);
});

test('VoskModelManager: initializes and manages recognizer lifecycle with custom grammar and Option A aliases', async () => {
    const manager = new VoskModelManager();
    assert.equal(manager.isReady(), false);

    const ready = await manager.initialize();
    assert.equal(ready, true);
    assert.equal(manager.isReady(), true);

    // Option A: Comma-separated list of wake words
    const rec = manager.createWakeWordRecognizer('ignite, bot', ['siguiente', 'pausa']);
    assert.ok(rec, 'Recognizer should be created successfully with multiple wake words');

    // Feed silence chunk (16kHz 16-bit mono = 3200 bytes for 100ms)
    const silence = Buffer.alloc(3200);
    const chunkRes = manager.processAudioChunk(rec, silence);

    assert.ok(typeof chunkRes.accepted === 'boolean');
    assert.ok(typeof chunkRes.partial === 'object');
    assert.ok(typeof chunkRes.result === 'object');

    // Test processUtterance using getFinalResult
    const utteranceRes = manager.processUtterance(rec, silence);
    assert.ok(typeof utteranceRes.text === 'string');
    assert.equal(utteranceRes.text, ''); // silence produces empty text

    // Test wrapper getFinalResult directly
    const directFinal = manager.wrapper.getFinalResult(rec);
    assert.ok(typeof directFinal.text === 'string');

    // Cleanup
    manager.freeRecognizer(rec);
    manager.destroy();
    assert.equal(manager.isReady(), false);
});

test('VoskModelManager: recognizer pool borrows, reuses, and resets recognizers cleanly', async () => {
    const manager = new VoskModelManager();
    await manager.initialize();

    const targetWords = parseWakeWords('ignite, bot');
    const grammarKey = manager.getGrammarKey(targetWords);

    // 1. First acquire creates a new recognizer
    const rec1 = manager.acquireRecognizer('ignite, bot');
    assert.ok(rec1);

    // 2. Release returns it to the pool
    manager.releaseRecognizer(rec1);
    const pool = manager.recognizerPool.get(grammarKey);
    assert.ok(pool);
    assert.equal(pool.length, 1);

    // 3. Second acquire reuses the pooled recognizer
    const rec2 = manager.acquireRecognizer('ignite, bot');
    assert.equal(rec2, rec1, 'Pool should reuse the same recognizer instance');
    assert.equal(pool.length, 0);

    // 4. Release it back
    manager.releaseRecognizer(rec2);
    assert.equal(pool.length, 1);

    // 5. Destroy frees all pooled recognizers
    manager.destroy();
    assert.equal(manager.recognizerPool.size, 0);
    assert.equal(manager.isReady(), false);
});

test('Vosk: real speech audio fixture detects wake word "ignite"', async (t) => {
    if (!fs.existsSync(IGNITE_SAMPLE)) {
        t.skip('ignite-sample.wav fixture not present');
        return;
    }

    const manager = new VoskModelManager();
    await manager.initialize();

    const rec = manager.acquireRecognizer('ignite, bot');
    assert.ok(rec);

    const wavBuffer = fs.readFileSync(IGNITE_SAMPLE);
    const pcm16k = wavBuffer.subarray(44); // strip RIFF header

    const { text } = manager.processUtterance(rec, pcm16k);
    assert.ok(text.includes('ignite'), `Expected text to contain "ignite", received: "${text}"`);

    manager.releaseRecognizer(rec);
    manager.destroy();
});

test('Vosk: casual speech audio fixture rejects non-wake-word sentence', async (t) => {
    if (!fs.existsSync(CASUAL_SAMPLE)) {
        t.skip('casual-speech-sample.wav fixture not present');
        return;
    }

    const manager = new VoskModelManager();
    await manager.initialize();

    const targetWakeWords = parseWakeWords('ignite, bot');
    const rec = manager.acquireRecognizer(targetWakeWords);
    assert.ok(rec);

    const wavBuffer = fs.readFileSync(CASUAL_SAMPLE);
    const pcm16k = wavBuffer.subarray(44); // strip RIFF header

    const { text } = manager.processUtterance(rec, pcm16k);
    const wakeDetected = targetWakeWords.some((w) => text.includes(w));

    assert.equal(wakeDetected, false, `Casual speech should not trigger wake words, received: "${text}"`);

    manager.releaseRecognizer(rec);
    manager.destroy();
});

test('parseWakeWords: parses single strings, comma-separated lists, and arrays', () => {
    assert.deepEqual(parseWakeWords('ignite'), ['ignite']);
    assert.deepEqual(parseWakeWords('ignite, bot, asistente'), ['ignite', 'bot', 'asistente']);
    assert.deepEqual(parseWakeWords(['ignite', 'bot']), ['ignite', 'bot']);
    assert.deepEqual(parseWakeWords(''), ['ignite']);
    assert.deepEqual(parseWakeWords(null), ['ignite']);
});

test('Matcher with Option A and conversational fillers', () => {
    // 1. Matches secondary alias 'bot'
    const matchBot = matchVoiceCommand('bot siguiente', { wakeWord: 'ignite, bot' });
    assert.equal(matchBot.matched, true);
    assert.equal(matchBot.commandId, 'skip');
    assert.equal(matchBot.wakeWord, 'bot');

    // 2. Matches with conversational prelude ('oye ignite...')
    const matchFiller = matchVoiceCommand('¡Oye Ignite, reproduce Starboy!', { wakeWord: 'ignite, bot' });
    assert.equal(matchFiller.matched, true);
    assert.equal(matchFiller.commandId, 'play');
    assert.equal(matchFiller.query, 'starboy');
    assert.equal(matchFiller.wakeWord, 'ignite');

    // 3. Matches with 2-word conversational prelude ('por favor bot...')
    const matchTwoWords = matchVoiceCommand('por favor bot pausa', { wakeWord: 'ignite, bot' });
    assert.equal(matchTwoWords.matched, true);
    assert.equal(matchTwoWords.commandId, 'pause');
    assert.equal(matchTwoWords.wakeWord, 'bot');

    // 4. Rejects when wake word is missing
    const noWake = matchVoiceCommand('reproduce starboy', { wakeWord: 'ignite, bot' });
    assert.equal(noWake.matched, false);
    assert.equal(noWake.reason, 'wake_word_missing');

    // 5. Rejects only wake word
    const onlyWake = matchVoiceCommand('ignite', { wakeWord: 'ignite, bot' });
    assert.equal(onlyWake.matched, false);
    assert.equal(onlyWake.reason, 'only_wake_word');
});
