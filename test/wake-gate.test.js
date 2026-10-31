'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const { WakeGate, pcmRms, isLikelyHallucination } = require('../src/voice');
const { matchVoiceCommand } = require('../src/voice/matcher');

const FIXTURES = path.join(__dirname, 'fixtures');
const pcm = (name) => fs.readFileSync(path.join(FIXTURES, name)).subarray(44);

// Audio the gate must let through: wake word followed by a command.
const SHOULD_OPEN = [
    'wake-pos-ignite-pausa.wav',
    'wake-pos-oye-ignite-reproduce.wav',
    'wake-pos-ignite-pause-en.wav',
];

// Audio the gate must keep closed: no wake word, or a similar-sounding word.
const SHOULD_STAY_CLOSED = [
    'wake-neg-ignores.wav',
    'wake-neg-knight-night.wav',
    'wake-neg-excite.wav',
    'wake-neg-invite.wav',
    'wake-neg-spanish-chat.wav',
    'casual-speech-sample.wav',
];

async function readyGate(t) {
    const gate = new WakeGate();
    if (!(await gate.initialize())) {
        t.skip('Vosk binary or English model not available');
        return null;
    }
    return gate;
}

test('wakeGate: not ready -> closed with gate_unavailable (never opens by accident)', () => {
    const gate = new WakeGate();
    const res = gate.evaluate(Buffer.alloc(32000), 'ignite');
    assert.equal(res.passed, false);
    assert.equal(res.reason, 'gate_unavailable');
});

test('wakeGate: opens for wake word followed by a command', async (t) => {
    const gate = await readyGate(t);
    if (!gate) return;
    for (const file of SHOULD_OPEN) {
        const res = gate.evaluate(pcm(file), 'ignite');
        assert.equal(res.passed, true, `${file} should open the gate (reason: ${res.reason}, heard: "${res.heard}")`);
        assert.equal(res.wakeWord, 'ignite');
    }
    gate.destroy();
});

test('wakeGate: stays closed for speech without the wake word and for look-alike words', async (t) => {
    const gate = await readyGate(t);
    if (!gate) return;
    for (const file of SHOULD_STAY_CLOSED) {
        const res = gate.evaluate(pcm(file), 'ignite');
        assert.equal(res.passed, false, `${file} must not open the gate (heard: "${res.heard}")`);
    }
    gate.destroy();
});

test('wakeGate: wake word alone (no command after it) does not call Groq', async (t) => {
    const gate = await readyGate(t);
    if (!gate) return;
    const res = gate.evaluate(pcm('ignite-sample.wav'), 'ignite');
    assert.equal(res.passed, false);
    assert.equal(res.reason, 'no_command_after');
    gate.destroy();
});

test('wakeGate: wake words missing from the model dictionary are reported, not guessed', async (t) => {
    const gate = await readyGate(t);
    if (!gate) return;
    const res = gate.evaluate(pcm('wake-pos-ignite-pausa.wav'), 'zxqvyw');
    assert.equal(res.passed, false);
    assert.equal(res.reason, 'no_supported_wake_word');
    assert.deepEqual(res.unsupported, ['zxqvyw']);

    // "bot" is not added behind the guild's back
    const botOnly = gate.buildGrammar('ignite').plans[0].grammar;
    assert.equal(botOnly.includes('bot'), false);
    assert.equal(botOnly.includes('[unk]'), true);
    assert.equal(botOnly.includes('night'), true, 'decoys must be part of the grammar');
    gate.destroy();
});

test('pcmRms: silence is 0 and speech is far above the default threshold', () => {
    assert.equal(pcmRms(Buffer.alloc(32000)), 0);
    assert.equal(pcmRms(Buffer.alloc(0)), 0);
    assert.ok(pcmRms(pcm('ignite-sample.wav')) > 1000);
});

test('isLikelyHallucination: catches invented phrases and Whisper no-speech segments', () => {
    assert.equal(isLikelyHallucination(''), true);
    assert.equal(isLikelyHallucination('Gracias.'), true);
    assert.equal(isLikelyHallucination('¡Suscríbete!'), true);
    assert.equal(isLikelyHallucination('Subtítulos por la comunidad de Amara.org'), true);
    assert.equal(isLikelyHallucination('Thank you.'), true);
    assert.equal(isLikelyHallucination('ignite pausa'), false);

    const silent = [{ no_speech_prob: 0.9, avg_logprob: -1.5 }];
    assert.equal(isLikelyHallucination('ignite pon algo', silent), true);

    const real = [{ no_speech_prob: 0.05, avg_logprob: -0.2 }];
    assert.equal(isLikelyHallucination('ignite pon algo', real), false);

    // One real segment is enough to keep the text
    assert.equal(isLikelyHallucination('ignite pon algo', [...silent, ...real]), false);
});

test('matcher: wake word may be anywhere, but the command must follow it directly', () => {
    const opts = { wakeWord: 'ignite' };
    assert.equal(matchVoiceCommand('ayer le dije a mi amigo, ignite pon bad bunny', opts).commandId, 'play');
    assert.equal(matchVoiceCommand('oye ignite reproduce starboy', opts).query, 'starboy');

    assert.equal(matchVoiceCommand('ignite', opts).reason, 'only_wake_word');
    assert.equal(matchVoiceCommand('ignite un dia de estos pon musica', opts).matched, false);
    assert.equal(matchVoiceCommand('ayer en la night pon algo de reggaeton', opts).matched, false);
    assert.equal(matchVoiceCommand('el bot dale', opts).matched, false);
});
