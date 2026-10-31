'use strict';

/**
 * Admission control for a captured utterance. Decides if it may reach Groq:
 *   0. volume: drop near-silent audio
 *   1. wake-word gate: the wake word must be heard, followed by something
 *   2. cooldown: limit Groq requests per user
 * Every rejection that matters is written to the voice event log.
 */

const config = require('../config');
const log = require('../core/logger').createLogger('voice-admission');
const { pcmRms } = require('./audio-transcoder');
const { wakeGate } = require('./wake-gate');
const { voiceSettingsStore } = require('./settings-store');
const { voiceEventLogStore } = require('./event-log');

/**
 * @param {object} params
 * @param {string} params.guildId
 * @param {string} params.userId
 * @param {Buffer} params.wavBuffer - 16 kHz mono WAV (44-byte header).
 * @param {object} [params.options] - Receiver options (offlineGating, wakeWord).
 * @param {Map<string, number>} params.lastGroqAt - Last Groq request time per guild:user.
 * @returns {Promise<{ admitted: boolean, pcm16k: Buffer, gate: object|null }>}
 */
async function admitAudio({ guildId, userId, wavBuffer, options = {}, lastGroqAt }) {
    const pcm16k = wavBuffer.subarray(44);
    const base = { guildId, userId, audioBytes: wavBuffer.length };

    // Stage 0: drop near-silent audio (keyboard, breathing, background hiss)
    const rms = pcmRms(pcm16k);
    if (rms < config.voice.minRms) {
        log.debug({ guildId, userId, rms }, 'Audio too quiet; discarded');
        return { admitted: false, pcm16k, gate: null };
    }

    // Stage 1: local wake-word gate. Groq is called ONLY if the gate opens.
    let gate = null;
    const offlineGating = options.offlineGating !== undefined ? options.offlineGating : config.voice.offlineGating;
    if (offlineGating) {
        await wakeGate.initialize();
        let wakeWordConfig = options.wakeWord;
        if (!wakeWordConfig) {
            const guildSettings = await voiceSettingsStore.getGuildSettings(guildId).catch(() => null);
            wakeWordConfig = guildSettings?.wake_word || config.voice.defaultWakeWord || 'ignite';
        }
        gate = wakeGate.evaluate(pcm16k, wakeWordConfig);
        if (!gate.passed) {
            log.info({ guildId, userId, gate: gate.reason, heard: gate.heard, unsupported: gate.unsupported }, 'Wake-word gate closed; Groq not called');
            voiceEventLogStore.record({
                ...base,
                voskHeard: gate.heard,
                wakeWordSpotted: gate.reason === 'no_command_after',
                matchedWakeWord: gate.wakeWord || null,
                groqEngaged: false,
                gateReason: gate.reason,
                status: 'filtered_out',
            });
            return { admitted: false, pcm16k, gate };
        }
        log.info({ guildId, userId, heard: gate.heard, wakeWord: gate.wakeWord, startMs: gate.startMs }, 'Wake-word gate open; engaging Groq');
    }

    // Stage 2: limit Groq calls per user
    const cooldownKey = `${guildId}:${userId}`;
    const now = Date.now();
    if (now - (lastGroqAt.get(cooldownKey) || 0) < config.voice.groqCooldownMs) {
        log.info({ guildId, userId }, 'Groq cooldown active; request skipped');
        voiceEventLogStore.record({
            ...base,
            voskHeard: gate?.heard || '',
            wakeWordSpotted: Boolean(gate?.passed),
            matchedWakeWord: gate?.wakeWord || null,
            groqEngaged: false,
            gateReason: 'rate_limited',
            status: 'rate_limited',
        });
        return { admitted: false, pcm16k, gate };
    }
    lastGroqAt.set(cooldownKey, now);

    return { admitted: true, pcm16k, gate };
}

module.exports = {
    admitAudio,
};
