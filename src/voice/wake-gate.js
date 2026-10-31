'use strict';

/**
 * Wake-word gate: cheap local check that runs BEFORE any Groq request.
 * A Vosk recognizer limited to a tiny grammar (wake phrase + similar-sounding decoys +
 * "[unk]") answers: "was the wake word said, and did something follow it?".
 *
 * - Anything outside the grammar becomes "[unk]", so normal speech cannot become a wake word.
 * - Decoys (night, knight, ignore...) win when a sound is closer to them than to the wake word.
 * - A wake word may be a phrase ("ignite bot") or a compound ("ignitebot", split into
 *   dictionary words). Each wake word uses the first model that knows all its words:
 *   English first (knows "ignite"), then Spanish if installed (knows Spanish-only words).
 * - The wake word may be anywhere in the clip, but speech must follow it: a recognized
 *   word or voiced audio. The matcher later checks that it is a real command.
 *
 * Vosk reports confidence 1.0 for every grammar word, so confidence is not used.
 */

const fs = require('node:fs');
const path = require('node:path');
const log = require('../core/logger').createLogger('wake-gate');
const { ensureVoskBinary, ensureEnglishModel, MODELS_DIR } = require('./model-downloader');
const { VoskWrapper } = require('./vosk-binding');
const { parseWakeWords } = require('./matcher');
const { pcmRms } = require('./audio-transcoder');

/** Words that sound like a wake word. They absorb near-misses. */
const WAKE_DECOYS = Object.freeze({
    ignite: ['night', 'knight', 'ignore', 'ignored', 'ignores', 'excite', 'invite'],
});

/** Words people say before the wake word. Without them "oye ignite" is often missed. */
const FILLER_WORDS = Object.freeze(['oye', 'hey']);

const UNKNOWN = '[unk]';
const MODEL_ORDER = ['en', 'es'];
const SPANISH_MODEL_DIR = path.join(MODELS_DIR, 'vosk-model-small-es');

/** Audio after the wake word counts as speech when a 100 ms window reaches this RMS. */
const TAIL_VOICE_RMS = 400;
const TAIL_SKIP_SEC = 0.1;
const TAIL_WINDOW_SAMPLES = 1600;

/** Splits "ignitebot" into ["ignite", "bot"] when both halves are dictionary words. */
function splitCompound(word, known) {
    for (let i = word.length - 2; i >= 2; i--) {
        const left = word.slice(0, i);
        const right = word.slice(i);
        if (known(left) && known(right)) return [left, right];
    }
    return null;
}

/** True when 100 ms of voiced audio exists after `fromSec`. */
function hasVoicedTail(pcm16k, fromSec) {
    const window = TAIL_WINDOW_SAMPLES * 2;
    for (let offset = Math.floor(fromSec * 16000) * 2; offset + window <= pcm16k.length; offset += window) {
        if (pcmRms(pcm16k.subarray(offset, offset + window)) >= TAIL_VOICE_RMS) return true;
    }
    return false;
}

class WakeGate {
    constructor() {
        /** @type {VoskWrapper|null} */
        this.wrapper = null;
        /** @type {Map<string, object>} model id ("en" | "es") -> loaded model */
        this.models = new Map();
        this.ready = false;
        /** @type {Promise<boolean>|null} */
        this.initPromise = null;
    }

    /**
     * Loads the native library, the English model and, if installed, the Spanish model.
     * @returns {Promise<boolean>}
     */
    initialize() {
        if (this.ready) return Promise.resolve(true);
        if (this.initPromise) return this.initPromise;

        this.initPromise = (async () => {
            try {
                const binaryPath = await ensureVoskBinary();
                const englishDir = binaryPath ? await ensureEnglishModel() : null;
                if (!binaryPath || !englishDir) {
                    log.warn('Wake-word gate unavailable: Vosk binary or English model missing');
                    return false;
                }
                this.wrapper = new VoskWrapper(binaryPath);
                this.models.set('en', this.wrapper.createModel(englishDir));
                if (fs.existsSync(path.join(SPANISH_MODEL_DIR, 'am', 'final.mdl'))) {
                    this.models.set('es', this.wrapper.createModel(SPANISH_MODEL_DIR));
                }
                this.ready = true;
                log.info({ models: [...this.models.keys()] }, 'Wake-word gate ready');
                return true;
            } catch (err) {
                log.error({ err: err.message }, 'Wake-word gate failed to initialize');
                this.ready = false;
                return false;
            } finally {
                this.initPromise = null;
            }
        })();

        return this.initPromise;
    }

    isReady() {
        return this.ready && this.models.size > 0 && this.wrapper !== null;
    }

    /** Dictionary words that make up `wakeWord` in one model, or null if it cannot be heard there. */
    tokensFor(wakeWord, modelId) {
        const model = this.models.get(modelId);
        const known = (w) => this.wrapper.hasWord(model, w);
        const tokens = [];
        for (const part of wakeWord.split(' ').filter(Boolean)) {
            const pieces = known(part) ? [part] : splitCompound(part, known);
            if (!pieces) return null;
            tokens.push(...pieces);
        }
        return tokens.length > 0 ? tokens : null;
    }

    /**
     * Builds one restricted grammar per model that is needed.
     * Wake words no model can hear are reported as unsupported.
     *
     * @param {string|string[]} wakeWordInput
     * @returns {{ plans: Array<{ modelId: string, grammar: string[], phrases: Array<{ wakeWord: string, tokens: string[] }> }>, supported: string[], unsupported: string[] }}
     */
    buildGrammar(wakeWordInput) {
        const plans = new Map();
        const supported = [];
        const unsupported = [];

        for (const wakeWord of parseWakeWords(wakeWordInput)) {
            const modelId = MODEL_ORDER.find((id) => this.models.has(id) && this.tokensFor(wakeWord, id));
            if (!modelId) {
                unsupported.push(wakeWord);
                continue;
            }
            supported.push(wakeWord);
            if (!plans.has(modelId)) plans.set(modelId, { modelId, entries: new Set(), phrases: [] });
            const plan = plans.get(modelId);
            const tokens = this.tokensFor(wakeWord, modelId);
            plan.entries.add(tokens.join(' '));
            plan.phrases.push({ wakeWord, tokens });
            const model = this.models.get(modelId);
            for (const decoy of WAKE_DECOYS[wakeWord] || []) {
                if (this.wrapper.hasWord(model, decoy)) plan.entries.add(decoy);
            }
        }

        const result = [...plans.values()].map((plan) => {
            const model = this.models.get(plan.modelId);
            for (const filler of FILLER_WORDS) {
                if (this.wrapper.hasWord(model, filler)) plan.entries.add(filler);
            }
            return { modelId: plan.modelId, grammar: [...plan.entries, UNKNOWN], phrases: plan.phrases };
        });

        return { plans: result, supported, unsupported };
    }

    /**
     * Runs one utterance through a fresh recognizer (creating one takes <1 ms).
     * Recognizers are not reused: after reset Vosk keeps counting time from the previous
     * clip, which would shift every word timestamp.
     */
    recognize(modelId, grammar, pcm16k) {
        const rec = this.wrapper.createRecognizer(this.models.get(modelId), 16000, grammar);
        try {
            this.wrapper.setWords(rec, true);
            this.wrapper.acceptWaveform(rec, pcm16k);
            return this.wrapper.getFinalResult(rec);
        } finally {
            this.wrapper.freeRecognizer(rec);
        }
    }

    /**
     * Decides if an utterance may be sent to Groq.
     *
     * @param {Buffer} pcm16k - Raw 16 kHz 16-bit mono PCM (no WAV header).
     * @param {string|string[]} wakeWordInput - Wake words configured for the guild.
     * @returns {{ passed: boolean, reason: string, heard: string, wakeWord?: string, startMs?: number, endMs?: number, unsupported?: string[] }}
     */
    evaluate(pcm16k, wakeWordInput) {
        if (!this.isReady()) {
            return { passed: false, reason: 'gate_unavailable', heard: '' };
        }

        const { plans, unsupported } = this.buildGrammar(wakeWordInput);
        if (plans.length === 0) {
            return { passed: false, reason: 'no_supported_wake_word', heard: '', unsupported };
        }

        let heard = '';
        let spotted = null;

        for (const plan of plans) {
            const final = this.recognize(plan.modelId, plan.grammar, pcm16k);
            heard = [heard, (final?.text || '').trim()].filter(Boolean).join(' | ');
            const words = Array.isArray(final?.result) ? final.result : [];

            for (const { wakeWord, tokens } of plan.phrases) {
                for (let i = 0; i + tokens.length <= words.length; i++) {
                    if (!tokens.every((token, k) => words[i + k].word === token)) continue;
                    const last = words[i + tokens.length - 1];
                    spotted = spotted || wakeWord;
                    const followed = i + tokens.length < words.length || hasVoicedTail(pcm16k, last.end + TAIL_SKIP_SEC);
                    if (followed) {
                        return {
                            passed: true,
                            reason: 'ok',
                            heard,
                            wakeWord,
                            startMs: Math.round(words[i].start * 1000),
                            endMs: Math.round(last.end * 1000),
                        };
                    }
                }
            }
        }

        if (spotted) return { passed: false, reason: 'no_command_after', heard, wakeWord: spotted };
        return { passed: false, reason: 'no_wake_word', heard };
    }

    destroy() {
        if (!this.wrapper) return;
        for (const model of this.models.values()) this.wrapper.freeModel(model);
        this.models.clear();
        this.ready = false;
    }
}

const wakeGate = new WakeGate();

module.exports = {
    WakeGate,
    wakeGate,
    WAKE_DECOYS,
    FILLER_WORDS,
};
