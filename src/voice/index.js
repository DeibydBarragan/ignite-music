'use strict';

/**
 * Voice Module entry point for ignite-music.
 * Coordinates audio capture, PCM transcoding, and Groq Whisper STT.
 */

const { downsample48kStereoTo16kMono, pcmToWav, opusStreamToWav } = require('./audio-transcoder');
const { transcribeAudio, GROQ_API_URL } = require('./groq-client');
const { VoiceReceiverManager, voiceReceiver } = require('./receiver');
const { matchVoiceCommand, DEFAULT_COMMAND_BINDINGS, COMMANDS_WITH_ARGUMENTS, normalizeText, parseWakeWords } = require('./matcher');
const { sendTtsFeedback, resolveFeedbackMessage, DEFAULT_TTS_TEMPLATES, interpolateTemplate } = require('./tts-feedback');
const { dispatchVoiceCommand } = require('./dispatcher');
const { VoiceSettingsStore, voiceSettingsStore } = require('./settings-store');
const { VoskWrapper, initVoskApi } = require('./vosk-binding');
const { VoskModelManager, voskModelManager } = require('./model-manager');
const { ensureVoskBinary, ensureVoskModel } = require('./model-downloader');

module.exports = {
    downsample48kStereoTo16kMono,
    pcmToWav,
    opusStreamToWav,
    transcribeAudio,
    GROQ_API_URL,
    VoiceReceiverManager,
    voiceReceiver,
    matchVoiceCommand,
    DEFAULT_COMMAND_BINDINGS,
    COMMANDS_WITH_ARGUMENTS,
    normalizeText,
    parseWakeWords,
    sendTtsFeedback,
    resolveFeedbackMessage,
    DEFAULT_TTS_TEMPLATES,
    interpolateTemplate,
    dispatchVoiceCommand,
    VoiceSettingsStore,
    voiceSettingsStore,
    VoskWrapper,
    initVoskApi,
    VoskModelManager,
    voskModelManager,
    ensureVoskBinary,
    ensureVoskModel,
};
