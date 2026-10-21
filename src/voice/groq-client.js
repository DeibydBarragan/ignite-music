'use strict';

/**
 * Groq Whisper Speech-to-Text API client.
 * Calls Groq's ultra-low-latency Whisper endpoint using native Node 22 fetch and FormData.
 */

const config = require('../config');
const log = require('../core/logger').createLogger('groq-stt');

const GROQ_API_URL = 'https://api.groq.com/openai/v1/audio/transcriptions';

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
    formData.append('response_format', 'json');
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
    GROQ_API_URL,
};
