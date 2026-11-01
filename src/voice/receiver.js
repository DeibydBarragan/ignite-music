'use strict';

/**
 * Voice Receiver Manager:
 * Manages per-guild Discord voice audio streams, applies Voice Activity Detection (VAD)
 * via AfterSilence, decodes and converts audio to WAV, and triggers Groq STT.
 */

const { EventEmitter } = require('node:events');
const { EndBehaviorType } = require('@discordjs/voice');
const config = require('../config');
const log = require('../core/logger').createLogger('voice-receiver');
const { opusStreamToWav, slicePcmWav } = require('./audio-transcoder');
const { isLikelyHallucination } = require('./groq-client');
const { transcribeUtterance } = require('./stt');
const { admitAudio } = require('./admission');
const { shouldListenTo } = require('./speaker-filter');
const { voskModelManager } = require('./model-manager');
const { wakeGate } = require('./wake-gate');
const { voiceEventLogStore } = require('./event-log');

class VoiceReceiverManager extends EventEmitter {
    constructor() {
        super();
        /** @type {Map<string, { connection: object, speakingHandler: Function, activeUsers: Set<string> }>} */
        this.listeners = new Map();
        /** @type {Function|null} */
        this.commandHandler = null;
        /** @type {import('discord.js').Client|null} */
        this.client = null;
        /** @type {Map<string, number>} Last Groq request time per guild:user */
        this.lastGroqAt = new Map();
    }

    /**
     * Stores the active Discord.js Client instance for bot/member discovery.
     * @param {import('discord.js').Client} client
     */
    setClient(client) {
        this.client = client;
    }

    /**
     * Registers a callback function to handle transcribed voice input.
     * @param {Function} handler - Signature: async ({ guildId, userId, text, channelId, elapsedMs }) => void
     */
    setCommandHandler(handler) {
        this.commandHandler = handler;
    }

    /**
     * Attaches audio receiver listeners to an active Discord VoiceConnection.
     *
     * @param {string} guildId - Target guild ID.
     * @param {import('@discordjs/voice').VoiceConnection} connection - Active voice connection.
     * @param {object} [options]
     * @param {number} [options.silenceDurationMs] - Duration of silence to consider speech finished.
     * @param {string} [options.language='es'] - Language code for STT.
     * @param {string} [options.apiKey] - Optional custom Groq API key override.
     * @param {import('discord.js').Client} [options.client] - Discord client instance.
     */
    attach(guildId, connection, options = {}) {
        if (!config.voice.enabled && !options.enabled) {
            log.debug({ guildId }, 'Voice commands are disabled; skipping receiver attachment');
            return;
        }

        if (!connection || !connection.receiver) {
            log.warn({ guildId }, 'Cannot attach voice receiver: missing connection or receiver');
            return;
        }

        if (this.listeners.has(guildId)) {
            this.detach(guildId);
        }

        if (config.voice.enabled || options.enabled) {
            void wakeGate.initialize();
            void voskModelManager.initialize().catch((err) => {
                log.warn({ err: err.message }, 'Vosk offline model initialization failed; fallback to Groq-only mode');
            });
        }

        const activeUsers = new Set();
        const silenceDurationMs = options.silenceDurationMs || config.voice.silenceDurationMs || 1000;
        const language = options.language || 'es';
        const apiKey = options.apiKey || config.voice.groqApiKey;
        const client = options.client || this.client;

        const speakingHandler = async (userId) => {
            // Avoid capturing duplicate concurrent streams for the same user
            if (activeUsers.has(userId)) return;

            // Ignore this bot and other bots (see speaker-filter.js)
            if (!(await shouldListenTo({ client, guildId, userId, allowBotInput: options.allowBotInput }))) return;

            activeUsers.add(userId);
            log.info({ guildId, userId }, 'User speaking in voice channel; subscribing to audio stream');

            try {
                const opusStream = connection.receiver.subscribe(userId, {
                    end: {
                        behavior: EndBehaviorType.AfterSilence,
                        duration: silenceDurationMs,
                    },
                });

                const wavBuffer = await opusStreamToWav(opusStream, {
                    minDurationMs: 650,
                    maxDurationMs: 12000,
                });

                if (!wavBuffer) {
                    log.info({ guildId, userId }, 'Audio too short or empty; discarded');
                    return;
                }

                log.info({ guildId, userId, byteLength: wavBuffer.length }, 'WAV audio captured from user');

                // Stages 0-2: volume, wake-word gate and per-user cooldown (see admission.js)
                const { admitted, pcm16k, gate } = await admitAudio({
                    guildId, userId, wavBuffer, options, lastGroqAt: this.lastGroqAt,
                });
                if (!admitted) return;

                // Audio Slicing: trim leading chatter before the wake word to accelerate Groq inference
                let groqWav = wavBuffer;
                let groqPcm = pcm16k;
                if (gate?.startMs && gate.startMs > 0) {
                    const sliced = slicePcmWav(pcm16k, gate.startMs, 250);
                    groqWav = sliced.wavBuffer;
                    groqPcm = sliced.pcm16k;
                    log.info({
                        guildId,
                        userId,
                        startMs: gate.startMs,
                        originalBytes: wavBuffer.length,
                        slicedBytes: groqWav.length,
                        savedMs: Math.round((wavBuffer.length - groqWav.length) / 32),
                    }, 'Trimmed audio before wake word for faster Groq transcription');
                }

                // Stage 3: speech-to-text (Groq Whisper, local Vosk fallback)
                const { text, elapsedMs, segments: groqSegments } = await transcribeUtterance({
                    wavBuffer: groqWav, pcm16k: groqPcm, apiKey, language, prompt: options.prompt, guildId, userId,
                });

                if (!text || isLikelyHallucination(text, groqSegments)) {
                    log.info({ guildId, userId, text }, 'Empty or hallucinated transcription discarded');
                    if (text) {
                        voiceEventLogStore.record({
                            guildId,
                            userId,
                            audioBytes: groqWav.length,
                            voskHeard: gate?.heard || '',
                            wakeWordSpotted: Boolean(gate?.passed),
                            matchedWakeWord: gate?.wakeWord || null,
                            gateReason: gate?.reason || 'gate_disabled',
                            groqEngaged: true,
                            groqText: text,
                            groqElapsedMs: elapsedMs,
                            status: 'hallucination',
                        });
                    }
                    return;
                }

                const channelId = connection.joinConfig?.channelId || null;
                const payload = { guildId, userId, text, channelId, elapsedMs };
                this.emit('transcription', payload);

                let cmdRes = null;
                if (this.commandHandler) {
                    try {
                        cmdRes = await this.commandHandler(payload);
                    } catch (err) {
                        log.error({ err, guildId, userId }, 'Error executing voice command handler');
                        cmdRes = { executed: false, reason: 'execution_error', error: err.message };
                    }
                }

                voiceEventLogStore.record({
                    guildId,
                    userId,
                    audioBytes: groqWav.length,
                    voskHeard: gate?.heard || '',
                    wakeWordSpotted: Boolean(gate?.passed),
                    matchedWakeWord: gate?.wakeWord || null,
                    gateReason: gate?.reason || 'gate_disabled',
                    groqEngaged: true,
                    groqText: text,
                    groqElapsedMs: elapsedMs,
                    commandId: cmdRes?.commandId || null,
                    query: cmdRes?.query || null,
                    ttsMessage: cmdRes?.ttsMessage || null,
                    status: cmdRes?.executed ? 'executed' : (cmdRes?.reason || 'unmatched'),
                });
            } catch (err) {
                log.warn({ err: err.message, guildId, userId }, 'Failed to process voice stream');
                this.emit('error', { guildId, userId, error: err });
            } finally {
                activeUsers.delete(userId);
            }
        };

        connection.receiver.speaking.on('start', speakingHandler);

        this.listeners.set(guildId, {
            connection,
            speakingHandler,
            activeUsers,
        });

        log.info({ guildId }, 'Voice receiver attached successfully');
    }

    /**
     * Detaches voice listening from a guild.
     * @param {string} guildId
     */
    detach(guildId) {
        const listener = this.listeners.get(guildId);
        if (!listener) return;

        try {
            listener.connection.receiver?.speaking?.off('start', listener.speakingHandler);
            listener.activeUsers.clear();
        } catch (err) {
            log.warn({ err, guildId }, 'Error removing speaking listener');
        }

        this.listeners.delete(guildId);
        log.debug({ guildId }, 'Voice receiver detached');
    }

    /**
     * Detaches all active voice receivers across all guilds.
     */
    detachAll() {
        for (const guildId of this.listeners.keys()) {
            this.detach(guildId);
        }
    }
}

// Singleton instance
const voiceReceiver = new VoiceReceiverManager();

module.exports = {
    VoiceReceiverManager,
    voiceReceiver,
};
