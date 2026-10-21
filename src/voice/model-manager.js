'use strict';

/**
 * Model Manager Singleton:
 * Manages the lifecycle of the in-memory VoskModel instance,
 * initializes the native C FFI binding, and maintains a high-performance
 * pool of reusable recognizers in RAM to prevent C heap fragmentation.
 */

const log = require('../core/logger').createLogger('vosk-model');
const { ensureVoskBinary, ensureVoskModel } = require('./model-downloader');
const { VoskWrapper } = require('./vosk-binding');
const { parseWakeWords } = require('./matcher');

class VoskModelManager {
    constructor() {
        /** @type {VoskWrapper|null} */
        this.wrapper = null;
        /** @type {object|null} */
        this.model = null;
        /** @type {string|null} */
        this.modelPath = null;
        /** @type {boolean} */
        this.ready = false;
        /** @type {Promise<boolean>|null} */
        this.initPromise = null;
        /** @type {Map<string, any[]>} Keyed by grammar fingerprint */
        this.recognizerPool = new Map();
        /** @type {Map<any, string>} Maps pointer/handle to grammarKey */
        this.recognizerGrammarKeys = new Map();
        /** @type {number} Maximum idle recognizers kept per unique grammar */
        this.maxIdlePerGrammar = 4;
    }

    /**
     * Initializes the Vosk environment, downloads missing files if needed,
     * and loads the acoustic model into RAM.
     *
     * @returns {Promise<boolean>} True if ready, false otherwise.
     */
    async initialize() {
        if (this.ready) return true;
        if (this.initPromise) return this.initPromise;

        this.initPromise = (async () => {
            try {
                const binaryPath = await ensureVoskBinary();
                if (!binaryPath) {
                    log.warn('Native Vosk binary unavailable; offline wake-word detection disabled');
                    return false;
                }

                const modelDir = await ensureVoskModel();
                if (!modelDir) {
                    log.warn('Acoustic model unavailable; offline wake-word detection disabled');
                    return false;
                }

                this.wrapper = new VoskWrapper(binaryPath);
                this.model = this.wrapper.createModel(modelDir);
                this.modelPath = modelDir;
                this.ready = true;

                log.info({ modelDir }, 'Vosk acoustic model loaded successfully into memory');
                return true;
            } catch (err) {
                log.error({ err: err.message }, 'Failed to initialize Vosk model manager');
                this.ready = false;
                return false;
            } finally {
                this.initPromise = null;
            }
        })();

        return this.initPromise;
    }

    /**
     * Returns whether the Vosk model is loaded and ready.
     * @returns {boolean}
     */
    isReady() {
        return this.ready && this.model !== null && this.wrapper !== null;
    }

    /**
     * Deterministic fingerprint for a grammar combination.
     * @param {string[]} wakeWords
     * @param {string[]} [extraCommands=[]]
     * @returns {string}
     */
    getGrammarKey(wakeWords, extraCommands = []) {
        const sortedWake = [...wakeWords].sort();
        const sortedCmds = [...extraCommands].sort();
        return `${sortedWake.join(',')}|${sortedCmds.join(',')}`;
    }

    /**
     * Creates a new Vosk Recognizer instance with a constrained grammar
     * optimized for ultra-low latency wake-word spotting.
     *
     * @param {string|string[]} [wakeWordInput='ignite'] - Target wake word or comma-separated list of words.
     * @param {string[]} [extraCommands=[]] - Optional basic command words.
     * @returns {object|null}
     */
    createWakeWordRecognizer(wakeWordInput = 'ignite', extraCommands = []) {
        if (!this.isReady()) {
            return null;
        }

        const wakeWords = parseWakeWords(wakeWordInput);
        const grammarSet = new Set(['[unk]']);

        for (const word of wakeWords) {
            grammarSet.add(word);
        }

        for (const cmd of extraCommands) {
            const clean = String(cmd).toLowerCase().trim();
            if (clean && this.wrapper.hasWord(this.model, clean)) {
                grammarSet.add(clean);
            }
        }

        const grammar = Array.from(grammarSet);
        try {
            return this.wrapper.createRecognizer(this.model, 16000, grammar);
        } catch (err) {
            log.error({ err: err.message }, 'Failed to create wake word recognizer');
            return null;
        }
    }

    /**
     * Acquires an idle recognizer from the RAM pool or creates a new one if the pool is empty.
     *
     * @param {string|string[]} [wakeWordInput='ignite']
     * @param {string[]} [extraCommands=[]]
     * @returns {object|null}
     */
    acquireRecognizer(wakeWordInput = 'ignite', extraCommands = []) {
        if (!this.isReady()) {
            return null;
        }

        if (wakeWordInput === null) {
            const grammarKey = '__full__';
            const idleList = this.recognizerPool.get(grammarKey);
            if (idleList && idleList.length > 0) {
                return idleList.pop();
            }
            try {
                const rec = this.wrapper.createRecognizer(this.model, 16000, null);
                if (rec) this.recognizerGrammarKeys.set(rec, grammarKey);
                return rec;
            } catch (err) {
                log.error({ err: err.message }, 'Failed to create unconstrained recognizer');
                return null;
            }
        }

        const wakeWords = parseWakeWords(wakeWordInput);
        const grammarKey = this.getGrammarKey(wakeWords, extraCommands);

        const idleList = this.recognizerPool.get(grammarKey);
        if (idleList && idleList.length > 0) {
            return idleList.pop();
        }

        const rec = this.createWakeWordRecognizer(wakeWords, extraCommands);
        if (rec) {
            this.recognizerGrammarKeys.set(rec, grammarKey);
        }
        return rec;
    }

    /**
     * Resets a recognizer and returns it to the idle pool, or frees it from memory if the pool is full.
     *
     * @param {object} recognizer
     */
    releaseRecognizer(recognizer) {
        if (!recognizer) return;

        if (this.wrapper) {
            try {
                this.wrapper.reset(recognizer);
            } catch { /* ignore reset error */ }
        }

        const key = this.recognizerGrammarKeys.get(recognizer);
        if (key && this.ready) {
            if (!this.recognizerPool.has(key)) {
                this.recognizerPool.set(key, []);
            }
            const idleList = this.recognizerPool.get(key);
            if (idleList.length < this.maxIdlePerGrammar) {
                idleList.push(recognizer);
                return;
            }
        }

        this.freeRecognizer(recognizer);
    }

    /**
     * Frees a recognizer instance directly from C memory.
     * @param {object} recognizer
     */
    freeRecognizer(recognizer) {
        if (this.wrapper && recognizer) {
            this.recognizerGrammarKeys.delete(recognizer);
            this.wrapper.freeRecognizer(recognizer);
        }
    }

    /**
     * Feeds an entire audio utterance buffer into the recognizer and retrieves
     * the final decoded text without double-reading or prematurely consuming the result.
     *
     * @param {object} recognizer
     * @param {Buffer} pcm16k
     * @returns {{ text: string }}
     */
    processUtterance(recognizer, pcm16k) {
        if (!this.wrapper || !recognizer || !Buffer.isBuffer(pcm16k) || pcm16k.length === 0) {
            return { text: '' };
        }

        this.wrapper.acceptWaveform(recognizer, pcm16k);
        const finalResult = this.wrapper.getFinalResult(recognizer);
        this.wrapper.reset(recognizer);

        return {
            text: (finalResult.text || '').toLowerCase().trim(),
        };
    }

    /**
     * Feeds audio buffer into recognizer and checks for wake word.
     * @param {object} recognizer
     * @param {Buffer} pcm16k
     * @returns {{ accepted: boolean, result: { text?: string }, partial: { partial?: string } }}
     */
    processAudioChunk(recognizer, pcm16k) {
        if (!this.wrapper || !recognizer) {
            return { accepted: false, result: { text: '' }, partial: { partial: '' } };
        }

        const accepted = Boolean(this.wrapper.acceptWaveform(recognizer, pcm16k));
        const partial = this.wrapper.getPartialResult(recognizer);
        const result = accepted ? this.wrapper.getResult(recognizer) : { text: '' };

        return { accepted, result, partial };
    }

    /**
     * Tears down the in-memory model and frees all pooled recognizers upon application shutdown.
     */
    destroy() {
        if (this.wrapper) {
            for (const [, idleList] of this.recognizerPool) {
                for (const rec of idleList) {
                    try { this.wrapper.freeRecognizer(rec); } catch { /* ignore */ }
                }
            }
            this.recognizerPool.clear();
            this.recognizerGrammarKeys.clear();

            if (this.model) {
                this.wrapper.freeModel(this.model);
                this.model = null;
                this.ready = false;
                log.debug('Vosk model and pooled recognizers freed from memory');
            }
        }
    }
}

const voskModelManager = new VoskModelManager();

module.exports = {
    VoskModelManager,
    voskModelManager,
    parseWakeWords,
};
