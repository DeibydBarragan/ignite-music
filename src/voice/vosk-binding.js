'use strict';

/**
 * Vosk C FFI Bindings using Koffi:
 * Provides a high-performance, non-blocking interface to libvosk.dll
 * without requiring legacy node-gyp C++ compiler toolchains.
 */

const koffi = require('koffi');
const log = require('../core/logger').createLogger('vosk-binding');

let lib = null;
let api = null;

function initVoskApi(libraryPath) {
    if (api) return api;

    log.debug({ libraryPath }, 'Loading libvosk with Koffi');
    lib = koffi.load(libraryPath);

    const VoskModel = koffi.opaque('VoskModel');
    const VoskRecognizer = koffi.opaque('VoskRecognizer');

    api = {
        VoskModel,
        VoskRecognizer,
        vosk_set_log_level: lib.func('void vosk_set_log_level(int log_level)'),
        vosk_model_new: lib.func('VoskModel *vosk_model_new(const char *model_path)'),
        vosk_model_free: lib.func('void vosk_model_free(VoskModel *model)'),
        vosk_model_find_word: lib.func('int vosk_model_find_word(VoskModel *model, const char *word)'),
        vosk_recognizer_new: lib.func('VoskRecognizer *vosk_recognizer_new(VoskModel *model, float sample_rate)'),
        vosk_recognizer_new_grm: lib.func('VoskRecognizer *vosk_recognizer_new_grm(VoskModel *model, float sample_rate, const char *grammar)'),
        vosk_recognizer_free: lib.func('void vosk_recognizer_free(VoskRecognizer *recognizer)'),
        vosk_recognizer_accept_waveform: lib.func('int vosk_recognizer_accept_waveform(VoskRecognizer *recognizer, const uint8_t *data, int length)'),
        vosk_recognizer_result: lib.func('const char *vosk_recognizer_result(VoskRecognizer *recognizer)'),
        vosk_recognizer_partial_result: lib.func('const char *vosk_recognizer_partial_result(VoskRecognizer *recognizer)'),
        vosk_recognizer_final_result: lib.func('const char *vosk_recognizer_final_result(VoskRecognizer *recognizer)'),
        vosk_recognizer_reset: lib.func('void vosk_recognizer_reset(VoskRecognizer *recognizer)'),
    };

    // Suppress verbose kaldi engine logs
    api.vosk_set_log_level(-1);

    return api;
}

class VoskWrapper {
    constructor(libraryPath) {
        this.api = initVoskApi(libraryPath);
    }

    createModel(modelPath) {
        const model = this.api.vosk_model_new(modelPath);
        if (!model) {
            throw new Error(`Failed to load Vosk model from "${modelPath}"`);
        }
        return model;
    }

    freeModel(model) {
        if (model) {
            this.api.vosk_model_free(model);
        }
    }

    hasWord(model, word) {
        if (!model || !word) return false;
        return this.api.vosk_model_find_word(model, String(word).toLowerCase().trim()) >= 0;
    }

    createRecognizer(model, sampleRate = 16000, grammar = null) {
        if (!model) throw new Error('VoskModel is required to create a Recognizer');

        let recognizer = null;
        if (grammar && Array.isArray(grammar)) {
            const grammarJson = JSON.stringify(grammar);
            recognizer = this.api.vosk_recognizer_new_grm(model, sampleRate, grammarJson);
        } else {
            recognizer = this.api.vosk_recognizer_new(model, sampleRate);
        }

        if (!recognizer) {
            throw new Error('Failed to create VoskRecognizer');
        }
        return recognizer;
    }

    freeRecognizer(recognizer) {
        if (recognizer) {
            this.api.vosk_recognizer_free(recognizer);
        }
    }

    acceptWaveform(recognizer, buffer) {
        if (!recognizer || !Buffer.isBuffer(buffer) || buffer.length === 0) return 0;
        return this.api.vosk_recognizer_accept_waveform(recognizer, buffer, buffer.length);
    }

    getResult(recognizer) {
        if (!recognizer) return { text: '' };
        const raw = this.api.vosk_recognizer_result(recognizer);
        try {
            return JSON.parse(raw);
        } catch {
            return { text: '' };
        }
    }

    getPartialResult(recognizer) {
        if (!recognizer) return { partial: '' };
        const raw = this.api.vosk_recognizer_partial_result(recognizer);
        try {
            return JSON.parse(raw);
        } catch {
            return { partial: '' };
        }
    }

    getFinalResult(recognizer) {
        if (!recognizer) return { text: '' };
        const raw = this.api.vosk_recognizer_final_result(recognizer);
        try {
            return JSON.parse(raw);
        } catch {
            return { text: '' };
        }
    }

    reset(recognizer) {
        if (recognizer) {
            this.api.vosk_recognizer_reset(recognizer);
        }
    }
}

module.exports = {
    VoskWrapper,
    initVoskApi,
};
