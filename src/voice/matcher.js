'use strict';

/**
 * Voice Command Matcher:
 * Cleans and normalizes transcriptions, validates the wake word (including aliases,
 * conversational fillers, and words spoken before the wake word), and matches trigger
 * phrases to target command identifiers.
 */

const config = require('../config');

/**
 * Standard default voice command bindings.
 */
const DEFAULT_COMMAND_BINDINGS = Object.freeze({
    play: ['reproduce', 'reproducir', 'reproduceme', 'reproduced', 'pon', 'poner', 'ponme', 'toca', 'tocar', 'tocame', 'play'],
    play_next: [
        'reproduce despues', 'reproduce después',
        'pon despues', 'pon después',
        'toca despues', 'toca después',
        'pon a continuacion', 'pon a continuación',
        'añade a continuacion', 'añade a continuación',
        'agrega a continuacion', 'agrega a continuación',
        'añade despues', 'añade después',
        'agrega despues', 'agrega después',
        'siguiente cancion', 'play next',
    ],
    enqueue: ['agrega', 'agregar', 'agregame', 'añade', 'añadir', 'añademe', 'encola', 'encolar'],
    skip: ['siguiente', 'salta', 'saltar', 'pasa', 'pasar', 'skip', 'next'],
    pause: ['pausa', 'pausar', 'espera'],
    resume: ['continua', 'continuar', 'reanuda', 'reanudar', 'sigue', 'dale', 'resume'],
    stop: ['deten', 'detén', 'detener', 'apaga', 'apagar', 'stop'],
    previous: ['anterior', 'atras', 'atrás', 'regresa', 'volver'],
    shuffle: ['mezcla', 'mezclar', 'aleatorio', 'random', 'shuffle'],
    autoplay_on: [
        'activa autoplay', 'activa el autoplay', 'activa el modo autoplay', 'activa modo autoplay',
        'activa auto play', 'activa el auto play', 'activa el modo auto play', 'activa modo auto play',
        'enciende autoplay', 'enciende el autoplay', 'enciende el modo autoplay',
        'enciende auto play', 'enciende el auto play', 'enciende el modo auto play',
        'prende autoplay', 'prende el autoplay', 'prende el modo autoplay',
        'prende auto play', 'prende el auto play', 'prende el modo auto play',
        'activa la reproduccion automatica', 'activa reproduccion automatica',
        'enciende la reproduccion automatica', 'enciende reproduccion automatica',
        'autoplay on', 'autoplay activado',
    ],
    autoplay_off: [
        'desactiva autoplay', 'desactiva el autoplay', 'desactiva el modo autoplay', 'desactiva modo autoplay',
        'desactiva auto play', 'desactiva el auto play', 'desactiva el modo auto play', 'desactiva modo auto play',
        'apaga autoplay', 'apaga el autoplay', 'apaga el modo autoplay',
        'apaga auto play', 'apaga el auto play', 'apaga el modo auto play',
        'quita autoplay', 'quita el autoplay', 'quita el modo autoplay',
        'quita auto play', 'quita el auto play', 'quita el modo auto play',
        'desactiva la reproduccion automatica', 'desactiva reproduccion automatica',
        'apaga la reproduccion automatica', 'apaga reproduccion automatica',
        'autoplay off', 'autoplay desactivado',
    ],
    volume: ['volumen al', 'volumen a', 'volumen', 'sube el volumen a', 'baja el volumen a'],
    queue: ['cola', 'lista', 'fila', 'canciones'],
});

/**
 * Commands that expect an argument (e.g. song name, volume level).
 */
const COMMANDS_WITH_ARGUMENTS = new Set(['play', 'play_next', 'enqueue', 'volume']);

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
        .replace(/[¿?¡!.,;:()[\]{}"'\\/~_#*<>—–-]/g, ' ')
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

/**
 * Spellings Whisper may produce for a wake word. Only invented spellings belong here:
 * real words (night, but, book, box...) would turn normal speech into commands.
 */
const WAKE_WORD_PHONETIC_VARIANTS = Object.freeze({
    ignite: [
        'ignait', 'ignaite', 'inight', 'inayt', 'ignit',
        'ignayt', 'inite', 'ignete',
    ],
});

/**
 * Expands candidate wake words with their phonetic variants, sorted by length descending.
 * @param {string[]} candidateWakeWords
 * @returns {string[]}
 */
function getAllWakeWords(candidateWakeWords) {
    const set = new Set(candidateWakeWords.map((w) => w.toLowerCase()));
    for (const w of Array.from(set)) {
        const variants = WAKE_WORD_PHONETIC_VARIANTS[w];
        if (variants) {
            for (const v of variants) set.add(v);
        }
        // Whisper may write a compound as two words ("ignitebot" -> "ignite bot") and the reverse
        if (w.includes(' ')) {
            set.add(w.replace(/\s+/g, ''));
        } else if (w.length >= 8) {
            for (let i = 3; i <= w.length - 3; i++) set.add(`${w.slice(0, i)} ${w.slice(i)}`);
        }
    }
    return Array.from(set).sort((a, b) => b.length - a.length);
}

/**
 * Finds all wake word occurrences in normalized text anywhere in the sentence.
 *
 * @param {string} text - Normalized full transcription.
 * @param {string[]} candidateWakeWords - Normalized list of candidate wake words.
 * @returns {Array<{ wakeWord: string, prefix: string, rest: string, index: number }>}
 */
function findWakeWordOccurrences(text, candidateWakeWords) {
    if (!text || typeof text !== 'string') return [];
    const all = getAllWakeWords(candidateWakeWords);
    if (all.length === 0) return [];

    const pattern = new RegExp(`\\b(${all.map(escapeRegex).join('|')})\\b`, 'gi');
    const matches = [];
    let m;
    while ((m = pattern.exec(text)) !== null) {
        const wakeWord = m[1].toLowerCase();
        const prefix = text.slice(0, m.index).trim();
        const rest = text.slice(m.index + m[0].length).trim();
        matches.push({ wakeWord, prefix, rest, index: m.index });
    }
    return matches;
}

/**
 * Finds a matching wake word in normalized text (returns first occurrence or empty).
 * Compatible with Vosk receiver check.
 *
 * @param {string} text - Normalized full transcription.
 * @param {string[]} candidateWakeWords - Normalized list of candidate wake words.
 * @returns {{ matched: boolean, wakeWord?: string, rest?: string, prefix?: string }}
 */
function findWakeWordMatch(text, candidateWakeWords) {
    const occurrences = findWakeWordOccurrences(text, candidateWakeWords);
    if (occurrences.length > 0) {
        return {
            matched: true,
            wakeWord: occurrences[0].wakeWord,
            rest: occurrences[0].rest,
            prefix: occurrences[0].prefix,
        };
    }
    return { matched: false };
}

/**
 * Cleans conversational connector words from the beginning of search queries.
 * @param {string} query
 * @returns {string}
 */
function cleanQuery(query) {
    if (!query || typeof query !== 'string') return '';
    let q = query.trim();
    q = q.replace(/^(?:la\s+cancion\s+(?:de\s+)?|el\s+tema\s+(?:de\s+)?|la\s+musica\s+(?:de\s+)?|a\s+|de\s+|por\s+)/i, '');
    return q.trim();
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

const ALLOWED_NON_ARG_SUFFIXES = new Set([
    '',
    'por favor',
    'porfa',
    'favor',
    'gracias',
    'muchas gracias',
    'ya',
    'ahora',
    'de una',
    'la cancion',
    'esta cancion',
    'el tema',
    'este tema',
    'la musica',
    'la pista',
    'la reproduccion',
    'la lista',
    'la cola',
    'la cancion por favor',
    'esta cancion por favor',
    'la cancion ya',
    'la musica por favor',
    'la musica ya',
]);

function isAllowedNonArgQuery(query) {
    if (!query) return true;
    const normalized = query.toLowerCase().trim();
    return ALLOWED_NON_ARG_SUFFIXES.has(normalized);
}

function isPrepositionPara(phrase, query) {
    if (phrase !== 'para' || !query) return false;
    const firstWord = query.split(/\s+/)[0];
    return (
        firstWord.endsWith('ar') ||
        firstWord.endsWith('er') ||
        firstWord.endsWith('ir') ||
        firstWord.endsWith('le') ||
        firstWord.endsWith('les') ||
        firstWord.endsWith('me') ||
        firstWord.endsWith('te') ||
        firstWord.endsWith('se') ||
        firstWord.endsWith('nos') ||
        ['que', 'mi', 'ti', 'el', 'la', 'los', 'las', 'un', 'una', 'eso', 'esto', 'nada'].includes(firstWord)
    );
}

const COURTESY_PREFIXES = [
    'por favor puedes',
    'porfa puedes',
    'hazme el favor de',
    'hazme el favor y',
    'por favor',
    'porfa',
    'puedes',
    'favor de',
    'please',
    'can you',
];

/**
 * Checks if `text` begins directly with a command phrase from `phrases`.
 *
 * @param {string} text - Candidate text to inspect.
 * @param {Array<{ phrase: string, commandId: string }>} phrases - Ordered list of command phrases.
 * @returns {{ commandId: string, phrase: string, query: string, missingQuery: boolean } | null}
 */
function matchCommandAtStart(text, phrases) {
    if (!text) return null;
    for (const { phrase, commandId } of phrases) {
        if (text === phrase || text.startsWith(`${phrase} `)) {
            const rawQuery = text.slice(phrase.length).trim();
            const requiresArg = COMMANDS_WITH_ARGUMENTS.has(commandId);

            // Reject commands that do NOT take arguments if followed by arbitrary sentences
            if (!requiresArg && !isAllowedNonArgQuery(rawQuery)) {
                continue;
            }

            // Reject bare "para" when acting as a preposition (e.g. "para hacerle", "para que")
            if (isPrepositionPara(phrase, rawQuery)) {
                continue;
            }

            const query = requiresArg ? cleanQuery(rawQuery) : '';
            const missingQuery = requiresArg && !query;

            return {
                commandId,
                phrase,
                query,
                missingQuery,
            };
        }
    }
    return null;
}

/**
 * Searches for a command phrase in `rest`.
 * The command MUST immediately follow the wake word, optionally preceded only by
 * a strict polite courtesy expression (e.g. "por favor", "puedes").
 * Arbitrary conversational sentences between the wake word and the command are rejected.
 *
 * @param {string} rest - Text following the wake word.
 * @param {Array<{ phrase: string, commandId: string }>} phrases - Ordered list of command phrases.
 * @returns {{ commandId: string, phrase: string, query: string, missingQuery: boolean, filler?: string } | null}
 */
function matchCommandInRest(rest, phrases) {
    if (!rest) return null;

    // 1. Direct match at word 0 (immediately follows the wake word)
    const directMatch = matchCommandAtStart(rest, phrases);
    if (directMatch) {
        return {
            ...directMatch,
            filler: '',
        };
    }

    // 2. Strict courtesy prefix immediately before the command (e.g. "por favor", "porfa", "puedes")
    for (const prefix of COURTESY_PREFIXES) {
        if (rest === prefix || rest.startsWith(`${prefix} `)) {
            const afterPrefix = rest.slice(prefix.length).trim();
            const courtesyMatch = matchCommandAtStart(afterPrefix, phrases);
            if (courtesyMatch) {
                return {
                    ...courtesyMatch,
                    filler: prefix,
                };
            }
        }
    }

    return null;
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
    // Only the guild's own wake words count. "bot" is no longer added automatically.
    const candidateWakeWords = parseWakeWords(options.wakeWord || config.voice.defaultWakeWord || 'ignite');

    if (!text) {
        return { matched: false, reason: 'empty_text', raw: rawText };
    }

    // 1. Locate all wake word occurrences in the text
    const occurrences = findWakeWordOccurrences(text, candidateWakeWords);
    if (occurrences.length === 0) {
        return {
            matched: false,
            reason: 'wake_word_missing',
            wakeWord: candidateWakeWords[0],
            raw: rawText,
        };
    }

    // 2. Build ordered command phrases
    const phrases = buildPhraseList(options.customBindings);

    // 3. Check each occurrence to find one that contains a valid command
    for (const occurrence of occurrences) {
        const found = matchCommandInRest(occurrence.rest, phrases);
        if (found) {
            return {
                matched: true,
                wakeWord: occurrence.wakeWord,
                commandId: found.commandId,
                phrase: found.phrase,
                query: found.query,
                missingQuery: found.missingQuery,
                filler: found.filler,
                raw: rawText,
            };
        }
    }

    // If no command matched in any occurrence, pick the most relevant wake word
    const last = occurrences[occurrences.length - 1];
    if (!last.rest) {
        return { matched: false, reason: 'only_wake_word', wakeWord: last.wakeWord, raw: rawText };
    }

    return {
        matched: false,
        reason: 'command_not_found',
        wakeWord: last.wakeWord,
        rest: last.rest,
        raw: rawText,
    };
}

module.exports = {
    DEFAULT_COMMAND_BINDINGS,
    COMMANDS_WITH_ARGUMENTS,
    WAKE_WORD_PHONETIC_VARIANTS,
    normalizeText,
    parseWakeWords,
    findWakeWordMatch,
    findWakeWordOccurrences,
    buildPhraseList,
    matchVoiceCommand,
};
