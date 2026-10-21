'use strict';

/**
 * Auditory feedback integration with ignite-sound:
 * Interpolates custom or default message templates and triggers TTS playback
 * in the Discord voice channel via ignite-sound REST API.
 */

const config = require('../config');
const log = require('../core/logger').createLogger('voice-tts');

const DEFAULT_TTS_TEMPLATES = Object.freeze({
    play: 'Reproduciendo {query}',
    enqueue: 'Añadida {query} a la lista',
    skip: 'Saltando canción',
    pause: 'Música pausada',
    resume: 'Reanudando música',
    stop: 'Reproducción detenida',
    previous: 'Canción anterior',
    shuffle: 'Lista en modo aleatorio',
    volume: 'Volumen ajustado al {query}',
    queue: 'Mostrando la lista',
});

/**
 * Replaces tokens like {query} inside the message template.
 *
 * @param {string} template
 * @param {object} params
 * @returns {string}
 */
function interpolateTemplate(template, params = {}) {
    if (!template || typeof template !== 'string') return '';
    return template.replace(/\{(\w+)\}/g, (match, key) => {
        return params[key] !== undefined && params[key] !== null ? String(params[key]) : '';
    }).trim();
}

/**
 * Resolves the final confirmation text for a given command.
 *
 * @param {string} commandId
 * @param {object} [params]
 * @param {object} [customTemplates]
 * @returns {string|null}
 */
function resolveFeedbackMessage(commandId, params = {}, customTemplates = null) {
    const templates = {
        ...DEFAULT_TTS_TEMPLATES,
        ...(customTemplates && typeof customTemplates === 'object' ? customTemplates : {}),
    };

    const template = templates[commandId];
    if (!template) return null;

    return interpolateTemplate(template, params);
}

/**
 * Sends a TTS speech request to ignite-sound.
 *
 * @param {object} payload
 * @param {string} payload.guildId - Discord guild snowflake.
 * @param {string} payload.channelId - Discord voice channel snowflake.
 * @param {string} [payload.userId] - Discord user snowflake who spoke.
 * @param {string} payload.commandId - Command executed.
 * @param {object} [payload.params] - Parameters passed to command (e.g. { query }).
 * @param {object} [options]
 * @param {boolean} [options.enabled] - Override TTS feedback enabled state.
 * @param {object} [options.templates] - Custom templates mapping.
 * @param {string} [options.apiUrl] - ignite-sound API URL override.
 * @param {string} [options.apiSecret] - ignite-sound API Secret override.
 * @param {string} [options.voice] - Voice override (e.g. es-MX-DaliaNeural).
 * @returns {Promise<boolean>} True if dispatched successfully, false otherwise.
 */
async function sendTtsFeedback(payload, options = {}) {
    const isEnabled = options.enabled !== undefined ? options.enabled : config.voice.ttsFeedbackEnabled;
    if (!isEnabled) {
        return false;
    }

    const apiUrl = options.apiUrl || config.voice.soundApiUrl;
    if (!apiUrl) {
        log.debug('TTS feedback skipped: soundApiUrl is not configured');
        return false;
    }

    const { guildId, channelId, userId, commandId, params = {} } = payload;
    if (!guildId || !channelId) {
        log.debug('TTS feedback skipped: missing guildId or channelId');
        return false;
    }

    const message = resolveFeedbackMessage(commandId, params, options.templates);
    if (!message) {
        return false;
    }

    const secret = options.apiSecret || config.voice.soundApiSecret;
    const endpoint = `${apiUrl.replace(/\/+$/, '')}/api/v1/tts/speak`;

    try {
        const headers = { 'Content-Type': 'application/json' };
        if (secret) {
            headers.Authorization = `Bearer ${secret}`;
        }
        if (userId) {
            headers['X-User-Discord-Id'] = String(userId);
        }

        const body = {
            text: message,
            guildId,
            channelId,
        };
        if (userId) {
            body.userId = String(userId);
        }
        if (options.voice) {
            body.voice = options.voice;
        }

        const res = await fetch(endpoint, {
            method: 'POST',
            headers,
            body: JSON.stringify(body),
            signal: AbortSignal.timeout(5000),
        });

        if (!res.ok) {
            const errText = await res.text().catch(() => '');
            log.warn({ status: res.status, errText, guildId }, 'ignite-sound TTS returned non-200');
            return false;
        }

        log.info({ guildId, commandId, message }, 'TTS confirmation dispatched to ignite-sound');
        return true;
    } catch (err) {
        log.warn({ err: err.message, guildId }, 'Failed to send TTS confirmation to ignite-sound');
        return false;
    }
}

module.exports = {
    DEFAULT_TTS_TEMPLATES,
    interpolateTemplate,
    resolveFeedbackMessage,
    sendTtsFeedback,
};
