'use strict';

/**
 * Voice Command Matcher:
 * Cleans and normalizes transcriptions, validates the wake word (including aliases and
 * conversational fillers), and matches trigger phrases to target command identifiers.
 */

const config = require('../config');

/**
 * Standard default voice command bindings.
 */
const DEFAULT_COMMAND_BINDINGS = Object.freeze({
    play: ['reproduce', 'reproducir', 'pon', 'poner', 'toca', 'tocar', 'play'],
    enqueue: ['agrega', 'agregar', 'añade', 'añadir', 'encola', 'encolar'],
    skip: ['siguiente', 'salta', 'saltar', 'pasa', 'pasar', 'skip', 'next'],
    pause: ['pausa', 'pausar', 'para', 'espera'],
    resume: ['continua', 'continuar', 'reanuda', 'reanudar', 'sigue', 'dale', 'resume'],
    stop: ['deten', 'detén', 'detener', 'apaga', 'apagar', 'stop'],
    previous: ['anterior', 'atras', 'atrás', 'regresa', 'volver'],
    shuffle: ['mezcla', 'mezclar', 'aleatorio', 'random', 'shuffle'],
    volume: ['volumen al', 'volumen a', 'volumen', 'sube el volumen a', 'baja el volumen a'],
    queue: ['cola', 'lista', 'fila', 'canciones'],
});

/**
 * Commands that expect an argument (e.g. song name, volume level).
 */
const COMMANDS_WITH_ARGUMENTS = new Set(['play', 'enqueue', 'volume']);

/**
 * Escapes regex special characters.
 * @param {string} str
 * @returns {string}
 */
function escapeRegex(str) {
    return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function stripAccents(str) {
    return str
        .replace(/[áàäâ]/gi, 'a')
        .replace(/[éèëê]/gi, 'e')
        .replace(/[íìïî]/gi, 'i')
        .replace(/[óòöô]/gi, 'o')
        .replace(/[úùüû]/gi, 'u');
}

/**
 * Normalizes input text: lowercase, removes punctuation and collapses whitespace.
 *
 * @param {string} text
 * @returns {string}
 */
function normalizeText(text) {
    if (!text || typeof text !== 'string') return '';
    return stripAccents(text)
        .toLowerCase()
        .replace(/[¿?¡!.,;:()\[\]{}"'\\\/~_#*<>—–-]/g, ' ')
        .replace(/\s+/g, ' ')
        .trim();
}

/**
 * Parses a wake word string, comma-separated list, or array of strings into clean words.
 *
 * @param {string|string[]} [input]
 * @returns {string[]}
 */
function parseWakeWords(input) {
    if (!input) return ['ignite'];
    const rawList = Array.isArray(input) ? input : String(input).split(',');
    const cleaned = rawList
        .map((w) => normalizeText(w))
        .filter(Boolean);
    return cleaned.length > 0 ? cleaned : ['ignite'];
}

const WAKE_WORD_PHONETIC_VARIANTS = Object.freeze({
    ignite: [
        'ignait', 'inight', 'inayt', 'ignit', 'night', 'knight', 'inai', 'ig night',
        'i n aid', 'in aid', 'inaid', 'i naif', 'y naif', 'inaif', 'ynaif',
        'naif', 'nayf', 'naife', 'nayfe', 'naid', 'nayd',
        'i n hay', 'in hay', 'inhay', 'i n ay', 'in ay', 'inay',
        'y n hay', 'ynhay', 'y n ay', 'ynay',
        'y ahí', 'y ahi', 'y hay', 'y ay', 'y ai', 'y a i', 'y a y', 'y aid', 'y ait',
        'iahi', 'iahí', 'yahi', 'yahí',
        'ignayt', 'inite', 'ignete', 'ig nite'
    ],
});

/**
 * Finds a matching wake word in normalized text, allowing direct prefix or short conversational
 * prelude (e.g. "oye ignite...", "hey bot...").
 *
 * @param {string} text - Normalized full transcription.
 * @param {string[]} candidateWakeWords - Normalized list of candidate wake words.
 * @returns {{ matched: boolean, wakeWord?: string, rest?: string }}
 */
function findWakeWordMatch(text, candidateWakeWords) {
    const allCandidates = new Set(candidateWakeWords);
    for (const w of candidateWakeWords) {
        const variants = WAKE_WORD_PHONETIC_VARIANTS[w.toLowerCase()];
        if (variants) {
            for (const v of variants) allCandidates.add(v);
        }
    }
    const sorted = Array.from(allCandidates).sort((a, b) => b.length - a.length);

    for (const wakeWord of sorted) {
        if (!wakeWord) continue;

        if (text === wakeWord) {
            return { matched: true, wakeWord, rest: '' };
        }
        if (text.startsWith(`${wakeWord} `)) {
            return { matched: true, wakeWord, rest: text.slice(wakeWord.length).trim() };
        }

        // Allow up to 2 prelude words before the wake word (e.g. "oye", "hey", "hola", "por favor")
        const pattern = new RegExp(`^(?:(\\S+(?:\\s+\\S+)?)\\s+)?${escapeRegex(wakeWord)}(?:\\s+(.*))?$`);
        const match = text.match(pattern);
        if (match) {
            const prefix = match[1] || '';
            const rest = (match[2] || '').trim();
            const prefixWords = prefix.trim().split(/\s+/).filter(Boolean);
            if (prefixWords.length <= 2) {
                return { matched: true, wakeWord, rest };
            }
        }
    }

    return { matched: false };
}

/**
 * Builds a flat list of phrase bindings sorted by phrase length descending,
 * ensuring longer phrases (e.g. "volumen al") match before shorter prefixes ("volumen").
 *
 * @param {object} [customBindings] - Optional custom guild phrase bindings.
 * @returns {Array<{ phrase: string, commandId: string }>}
 */
function buildPhraseList(customBindings = null) {
    const list = [];

    // Add default bindings
    for (const [commandId, phrases] of Object.entries(DEFAULT_COMMAND_BINDINGS)) {
        for (const phrase of phrases) {
            list.push({ phrase: normalizeText(phrase), commandId });
        }
    }

    // Add / override custom bindings
    if (customBindings && typeof customBindings === 'object') {
        if (Array.isArray(customBindings)) {
            for (const item of customBindings) {
                if (item.phrase && item.commandId) {
                    list.push({ phrase: normalizeText(item.phrase), commandId: item.commandId });
                }
            }
        } else {
            for (const [cmd, phrases] of Object.entries(customBindings)) {
                if (Array.isArray(phrases)) {
                    for (const phrase of phrases) {
                        list.push({ phrase: normalizeText(phrase), commandId: cmd });
                    }
                } else if (typeof phrases === 'string') {
                    list.push({ phrase: normalizeText(phrases), commandId: cmd });
                }
            }
        }
    }

    // Sort descending by length
    return list.sort((a, b) => b.phrase.length - a.phrase.length);
}

/**
 * Parses and matches a transcribed text against wake word and voice commands.
 *
 * @param {string} rawText - Transcribed raw text from Groq.
 * @param {object} [options]
 * @param {string|string[]} [options.wakeWord] - Target wake word or comma-separated list of words.
 * @param {object} [options.customBindings] - Optional guild custom phrase bindings.
 * @returns {{
 *   matched: boolean,
 *   reason?: string,
 *   wakeWord?: string,
 *   commandId?: string,
 *   phrase?: string,
 *   query?: string,
 *   missingQuery?: boolean,
 *   raw: string
 * }}
 */
function matchVoiceCommand(rawText, options = {}) {
    const text = normalizeText(rawText);
    const candidateWakeWords = parseWakeWords(options.wakeWord || config.voice.defaultWakeWord || 'ignite');

    if (!text) {
        return { matched: false, reason: 'empty_text', raw: rawText };
    }

    // 1. Validate wake word presence (prefix or conversational prelude)
    const wakeMatch = findWakeWordMatch(text, candidateWakeWords);
    if (!wakeMatch.matched) {
        return {
            matched: false,
            reason: 'wake_word_missing',
            wakeWord: candidateWakeWords[0],
            raw: rawText,
        };
    }

    const { wakeWord, rest } = wakeMatch;
    if (!rest) {
        return { matched: false, reason: 'only_wake_word', wakeWord, raw: rawText };
    }

    // 2. Match against trigger phrases
    const phrases = buildPhraseList(options.customBindings);

    for (const { phrase, commandId } of phrases) {
        if (rest === phrase || rest.startsWith(`${phrase} `)) {
            const query = rest.slice(phrase.length).trim();
            const requiresArg = COMMANDS_WITH_ARGUMENTS.has(commandId);
            const missingQuery = requiresArg && !query;

            return {
                matched: true,
                wakeWord,
                commandId,
                phrase,
                query,
                missingQuery,
                raw: rawText,
            };
        }
    }

    return {
        matched: false,
        reason: 'command_not_found',
        wakeWord,
        rest,
        raw: rawText,
    };
}

module.exports = {
    DEFAULT_COMMAND_BINDINGS,
    COMMANDS_WITH_ARGUMENTS,
    normalizeText,
    parseWakeWords,
    findWakeWordMatch,
    buildPhraseList,
    matchVoiceCommand,
};
