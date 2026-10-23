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
const { opusStreamToWav } = require('./audio-transcoder');
const { transcribeAudio } = require('./groq-client');
const { voskModelManager, parseWakeWords } = require('./model-manager');

class VoiceReceiverManager extends EventEmitter {
    constructor() {
        super();
        /** @type {Map<string, { connection: object, speakingHandler: Function, activeUsers: Set<string> }>} */
        this.listeners = new Map();
        /** @type {Function|null} */
        this.commandHandler = null;
        /** @type {import('discord.js').Client|null} */
        this.client = null;
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

            // Filter out self (prevent infinite feedback loops)
            if (client?.user?.id && userId === client.user.id) {
                log.debug({ guildId, userId }, 'Ignoring audio from bot itself');
                return;
            }

            // Filter out other bots unless configured to allow bot voice input (e.g. testing via ignite-sound)
            if (client) {
                try {
                    const guild = client.guilds.cache.get(guildId) || await client.guilds.fetch(guildId).catch(() => null);
                    const member = guild?.members.cache.get(userId) || await guild?.members.fetch(userId).catch(() => null);
                    if (member?.user?.bot) {
                        const isSoundBot = Boolean(config.voice.soundBotId && String(userId) === String(config.voice.soundBotId));
                        const allowInput = Boolean(options.allowBotInput || (config.voice.allowBotInput && isSoundBot));
                        if (!allowInput) {
                            log.info({ guildId, userId }, 'Ignoring audio from bot member');
                            return;
                        }
                        log.info({ guildId, userId }, 'Accepting audio from sound bot for recognition');
                    }
                } catch {
                    // If member fetch fails, proceed cautiously
                }
            }

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

                const offlineGating = options.offlineGating !== undefined ? options.offlineGating : config.voice.offlineGating;

                // Stage 1: Local Offline Wake-Word Gating with Vosk (if enabled)
                if (offlineGating && voskModelManager.isReady()) {
                    const pcm16k = wavBuffer.subarray(44);
                    const wakeWordConfig = options.wakeWord || config.voice.defaultWakeWord || 'ignite';
                    const targetWakeWords = parseWakeWords(wakeWordConfig);
                    const rec = voskModelManager.acquireRecognizer(targetWakeWords);
                    if (rec) {
                        let wakeWordDetected = false;
                        let heardText = '';
                        try {
                            const res = voskModelManager.processUtterance(rec, pcm16k);
                            heardText = res.text;
                            wakeWordDetected = targetWakeWords.some((w) => heardText.includes(w));
                        } catch (err) {
                            log.warn({ err: err.message }, 'Vosk processing error; passing through');
                            wakeWordDetected = true; // Fail open on error
                        } finally {
                            voskModelManager.releaseRecognizer(rec);
                        }

                        if (!wakeWordDetected) {
                            log.info({ guildId, userId, heardText }, 'Vosk: casual speech filtered out');
                            return;
                        }
                        log.info({ guildId, userId, heardText, targetWakeWords }, 'Vosk: wake word spotted in local speech! Engaging Groq Cloud');
                    }
                }

                // Stage 2: STT (Groq Cloud Whisper with instant local Vosk fallback)
                let text = '';
                let elapsedMs = 0;

                if (apiKey) {
                    try {
                        const groqRes = await transcribeAudio(wavBuffer, {
                            apiKey,
                            language,
                        });
                        if (groqRes?.text) {
                            text = groqRes.text;
                            elapsedMs = groqRes.elapsedMs;
                            log.info({ guildId, userId, text, elapsedMs }, 'Voice input transcribed by Groq Whisper');
                        }
                    } catch (groqErr) {
                        log.warn({ err: groqErr.message, guildId, userId }, 'Groq STT failed; falling back to local Vosk');
                    }
                }

                // Fallback to local Vosk offline transcription
                if (!text && voskModelManager.isReady()) {
                    const startLocal = Date.now();
                    const rec = voskModelManager.acquireRecognizer(null);
                    if (rec) {
                        try {
                            const pcm16k = wavBuffer.subarray(44);
                            const res = voskModelManager.processUtterance(rec, pcm16k);
                            if (res?.text) {
                                text = res.text;
                                elapsedMs = Date.now() - startLocal;
                                log.info({ guildId, userId, text, elapsedMs }, 'Voice input transcribed by local Vosk');
                            }
                        } catch (err) {
                            log.warn({ err: err.message, guildId, userId }, 'Local Vosk transcription failed');
                        } finally {
                            voskModelManager.releaseRecognizer(rec);
                        }
                    }
                }

                const cleanText = (text || '').toLowerCase().trim().replace(/[.,!?;:]/g, '');
                if (!text || cleanText === 'gracias' || cleanText === 'muchas gracias' || cleanText === 'thank you') {
                    log.debug({ guildId, userId, text }, 'Empty or hallucination transcription discarded');
                    return;
                }

                const channelId = connection.joinConfig?.channelId || null;
                const payload = { guildId, userId, text, channelId, elapsedMs };
                this.emit('transcription', payload);

                if (this.commandHandler) {
                    await this.commandHandler(payload).catch((err) => {
                        log.error({ err, guildId, userId }, 'Error executing voice command handler');
                    });
                }
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
