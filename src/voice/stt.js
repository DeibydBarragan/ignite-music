'use strict';

/**
 * Speech-to-text for an admitted utterance:
 * Groq Whisper first, local Vosk (full vocabulary) as an offline fallback.
 */

const config = require('../config');
const log = require('../core/logger').createLogger('voice-stt');

const DEFAULT_PROMPT = 'Ignite, mierdabot, bot mierda, reproduce, pon, toca, pausa, salta, siguiente, anterior, agrega, añade, cola, lista, volumen, activa autoplay, desactiva autoplay, autoplay on, autoplay off.';
const { transcribeAudio } = require('./groq-client');
const { voskModelManager } = require('./model-manager');

/**
 * @param {object} params
 * @param {Buffer} params.wavBuffer - 16 kHz mono WAV.
 * @param {Buffer} params.pcm16k - Same audio without the WAV header.
 * @param {string} [params.apiKey] - Groq API key. Without it only Vosk is used.
 * @param {string} [params.language='es']
 * @param {string} params.guildId
 * @param {string} params.userId
 * @returns {Promise<{ text: string, elapsedMs: number, segments: object[] }>}
 */
async function transcribeUtterance({ wavBuffer, pcm16k, apiKey, language = 'es', prompt, guildId, userId }) {
    let text = '';
    let elapsedMs = 0;
    let segments = [];

    if (apiKey) {
        try {
            const promptToUse = prompt !== undefined ? prompt : (config.voice?.prompt || DEFAULT_PROMPT);
            const groqRes = await transcribeAudio(wavBuffer, { apiKey, language, prompt: promptToUse });
            if (groqRes?.text) {
                text = groqRes.text;
                elapsedMs = groqRes.elapsedMs;
                segments = groqRes.segments || [];
                log.info({ guildId, userId, text, elapsedMs }, 'Voice input transcribed by Groq Whisper');
            }
        } catch (groqErr) {
            log.warn({ err: groqErr.message, guildId, userId }, 'Groq STT failed; falling back to local Vosk');
        }
    }

    // Fallback to local Vosk offline transcription
    if (!text && voskModelManager.isReady()) {
        const startLocal = Date.now();
        const rec = voskModelManager.acquireRecognizer(null);
        if (rec) {
            try {
                const res = voskModelManager.processUtterance(rec, pcm16k);
                if (res?.text) {
                    text = res.text;
                    elapsedMs = Date.now() - startLocal;
                    log.info({ guildId, userId, text, elapsedMs }, 'Voice input transcribed by local Vosk');
                }
            } catch (err) {
                log.warn({ err: err.message, guildId, userId }, 'Local Vosk transcription failed');
            } finally {
                voskModelManager.releaseRecognizer(rec);
            }
        }
    }

    return { text, elapsedMs, segments };
}

module.exports = {
    transcribeUtterance,
};
