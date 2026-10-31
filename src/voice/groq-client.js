'use strict';

/**
 * Groq Whisper Speech-to-Text API client.
 * Calls Groq's ultra-low-latency Whisper endpoint using native Node 22 fetch and FormData.
 */

const config = require('../config');
const log = require('../core/logger').createLogger('groq-stt');

const GROQ_API_URL = 'https://api.groq.com/openai/v1/audio/transcriptions';

/** Whisper tends to invent these phrases on silence or noise. Compared after normalizing. */
const HALLUCINATION_PHRASES = new Set([
    'gracias', 'muchas gracias', 'gracias por ver el video', 'gracias por ver',
    'thank you', 'thanks', 'thanks for watching', 'thank you for watching',
    'suscribete', 'suscribete al canal', 'no olvides suscribirte',
    'subtitulos por la comunidad de amara org', 'subtitulado por la comunidad de amara org',
    'subtitulos realizados por la comunidad de amara org',
    'amara org', 'you', 'bye', 'adios', 'hasta la proxima',
]);

function normalizeForHallucination(text) {
    return String(text || '')
        .toLowerCase()
        .normalize('NFD')
        .replace(/[\u0300-\u036f]/g, '')
        .replace(/[^a-z0-9 ]+/g, ' ')
        .replace(/\s+/g, ' ')
        .trim();
}

/**
 * Detects text Whisper invented from silence or noise.
 * Uses a phrase list, and Whisper's own rule: a segment is "no speech" when
 * no_speech_prob > 0.6 AND avg_logprob < -1.
 *
 * @param {string} text
 * @param {Array<{ no_speech_prob?: number, avg_logprob?: number }>} [segments]
 * @returns {boolean}
 */
function isLikelyHallucination(text, segments = []) {
    const normalized = normalizeForHallucination(text);
    if (!normalized) return true;
    if (HALLUCINATION_PHRASES.has(normalized)) return true;
    if (!Array.isArray(segments) || segments.length === 0) return false;
    return segments.every((s) => (s.no_speech_prob ?? 0) > 0.6 && (s.avg_logprob ?? 0) < -1);
}

/**
 * Sends a WAV audio buffer to Groq's audio transcriptions endpoint.
 *
 * @param {Buffer} wavBuffer - Complete 16kHz mono WAV file buffer.
 * @param {object} [options]
 * @param {string} [options.apiKey] - Groq API key (defaults to config.voice.groqApiKey).
 * @param {string} [options.model] - Whisper model identifier.
 * @param {string} [options.language='es'] - ISO-639-1 language code.
 * @param {string} [options.prompt] - Optional prompt or context to guide the model.
 * @param {number} [options.timeoutMs=10000] - Request timeout in milliseconds.
 * @returns {Promise<{ text: string, duration?: number }>} Transcribed text result.
 */
async function transcribeAudio(wavBuffer, options = {}) {
    const apiKey = options.apiKey !== undefined ? options.apiKey : config.voice.groqApiKey;
    if (!apiKey) {
        throw new Error('Groq API Key is not configured. Set GROQ_API_KEY in .env or provide it in options.');
    }

    if (!Buffer.isBuffer(wavBuffer) || wavBuffer.length <= 44) {
        return { text: '' };
    }

    const model = options.model || config.voice.model || 'whisper-large-v3-turbo';
    const language = options.language !== undefined ? options.language : 'es';
    const timeoutMs = options.timeoutMs || 10000;

    const formData = new FormData();
    const blob = new Blob([wavBuffer], { type: 'audio/wav' });
    formData.append('file', blob, 'command.wav');
    formData.append('model', model);
    formData.append('response_format', 'verbose_json');
    formData.append('temperature', '0.0');

    if (language) {
        formData.append('language', language);
    }
    if (options.prompt) {
        formData.append('prompt', options.prompt);
    }

    const startTime = Date.now();
    log.info({ keyPrefix: apiKey ? apiKey.slice(0, 7) : null, keyLen: apiKey ? apiKey.length : 0, model }, 'Sending Groq transcription request');

    try {
        const response = await fetch(GROQ_API_URL, {
            method: 'POST',
            headers: {
                Authorization: `Bearer ${apiKey}`,
                'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
            },
            body: formData,
            signal: AbortSignal.timeout(timeoutMs),
        });

        if (!response.ok) {
            const errorBody = await response.text().catch(() => '');
            log.warn({ status: response.status, errorBody }, 'Groq transcription request failed');
            throw new Error(`Groq STT failed with status ${response.status}: ${errorBody}`);
        }

        const data = await response.json();
        const elapsedMs = Date.now() - startTime;
        const text = (data.text || '').trim();

        log.debug({ text, elapsedMs, model }, 'Groq transcription successful');

        return {
            text,
            elapsedMs,
            segments: Array.isArray(data.segments) ? data.segments : [],
        };
    } catch (err) {
        if (err.name === 'TimeoutError') {
            log.error({ timeoutMs }, 'Groq transcription request timed out');
            throw new Error(`Groq STT timed out after ${timeoutMs}ms`);
        }
        throw err;
    }
}

module.exports = {
    transcribeAudio,
    isLikelyHallucination,
    GROQ_API_URL,
};
