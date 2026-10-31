'use strict';

/**
 * Guild Voice Settings Store:
 * Manages caching and persistence in Supabase for guild wake words,
 * auditory feedback templates, and custom phrase bindings.
 */

const config = require('../config');
const log = require('../core/logger').createLogger('voice-settings');
const { supabase } = require('../core/supabase');
const { DEFAULT_TTS_TEMPLATES } = require('./tts-feedback');

const CACHE_TTL_MS = 60_000;

function extractCleanTemplates(rawTemplates) {
    if (!rawTemplates || typeof rawTemplates !== 'object') return {};
    const clean = {};
    for (const [key, value] of Object.entries(rawTemplates)) {
        if (!key.startsWith('_') && typeof value === 'string') {
            clean[key] = value;
        }
    }
    return clean;
}

class VoiceSettingsStore {
    constructor({ supabase: injectedSupabase } = {}) {
        this.supabase = injectedSupabase !== undefined ? injectedSupabase : supabase;
        /** @type {Map<string, { data: object, expiresAt: number }>} */
        this.cache = new Map();
    }

    /**
     * Fallback configuration based on .env and built-in defaults.
     */
    getDefaultSettings() {
        return {
            enabled: config.voice.enabled,
            wake_word: config.voice.defaultWakeWord || 'ignite',
            language: 'es',
            tts_feedback_enabled: config.voice.ttsFeedbackEnabled,
            tts_voice: 'es-MX-DaliaNeural',
            tts_pitch: 1.0,
            tts_rate: 1.0,
            tts_templates: { ...DEFAULT_TTS_TEMPLATES },
            bindings: [],
        };
    }

    /**
     * Retrieves the active voice settings and bindings for a guild.
     * Checks in-memory cache before querying Supabase.
     *
     * @param {string} guildId
     * @returns {Promise<object>}
     */
    async getGuildSettings(guildId) {
        if (!guildId) return this.getDefaultSettings();

        const cached = this.cache.get(guildId);
        if (cached && cached.expiresAt > Date.now()) {
            return cached.data;
        }

        if (!this.supabase) {
            return this.getDefaultSettings();
        }

        try {
            const [settingsRes, bindingsRes] = await Promise.all([
                this.supabase
                    .from('guild_voice_settings')
                    .select('*')
                    .eq('guild_id', guildId)
                    .maybeSingle(),
                this.supabase
                    .from('guild_voice_bindings')
                    .select('phrase, command_id')
                    .eq('guild_id', guildId),
            ]);

            if (settingsRes.error) {
                log.warn({ err: settingsRes.error.message, guildId }, 'Error loading guild_voice_settings from Supabase');
                return this.getDefaultSettings();
            }

            const rawSettings = settingsRes.data;
            const bindings = (bindingsRes.data || []).map((b) => ({
                phrase: b.phrase,
                command_id: b.command_id,
            }));

            const rawTemplates = rawSettings?.tts_templates || {};
            const cleanTemplates = extractCleanTemplates(rawTemplates);

            const result = {
                enabled: rawSettings?.enabled !== undefined ? rawSettings.enabled : config.voice.enabled,
                wake_word: (rawSettings?.wake_word || config.voice.defaultWakeWord || 'ignite').toLowerCase(),
                language: rawSettings?.language || 'es',
                tts_voice: rawTemplates._voice || 'es-MX-DaliaNeural',
                tts_pitch: rawTemplates._pitch !== undefined ? Number(rawTemplates._pitch) : 1.0,
                tts_rate: rawTemplates._rate !== undefined ? Number(rawTemplates._rate) : 1.0,
                tts_feedback_enabled: rawSettings?.tts_feedback_enabled !== undefined
                    ? rawSettings.tts_feedback_enabled
                    : config.voice.ttsFeedbackEnabled,
                tts_templates: {
                    ...DEFAULT_TTS_TEMPLATES,
                    ...cleanTemplates,
                },
                bindings,
            };

            this.cache.set(guildId, {
                data: result,
                expiresAt: Date.now() + CACHE_TTL_MS,
            });

            return result;
        } catch (err) {
            log.error({ err: err.message, guildId }, 'Failed to fetch guild voice settings');
            return this.getDefaultSettings();
        }
    }

    /**
     * Persists voice settings and bindings for a guild.
     *
     * @param {string} guildId
     * @param {object} payload
     * @returns {Promise<object>}
     */
    async updateGuildSettings(guildId, payload = {}) {
        const defaults = this.getDefaultSettings();
        const current = await this.getGuildSettings(guildId);

        const tts_voice = payload.tts_voice || current.tts_voice || 'es-MX-DaliaNeural';
        const tts_pitch = payload.tts_pitch !== undefined ? Number(payload.tts_pitch) : (current.tts_pitch !== undefined ? current.tts_pitch : 1.0);
        const tts_rate = payload.tts_rate !== undefined ? Number(payload.tts_rate) : (current.tts_rate !== undefined ? current.tts_rate : 1.0);

        const cleanTemplates = {
            ...defaults.tts_templates,
            ...extractCleanTemplates(current.tts_templates),
            ...extractCleanTemplates(payload.tts_templates),
        };

        const updated = {
            tts_voice,
            tts_pitch,
            tts_rate,
            enabled: payload.enabled !== undefined ? Boolean(payload.enabled) : current.enabled,
            wake_word: payload.wake_word ? payload.wake_word.trim().toLowerCase() : current.wake_word,
            language: payload.language || current.language,
            tts_feedback_enabled: payload.tts_feedback_enabled !== undefined
                ? Boolean(payload.tts_feedback_enabled)
                : current.tts_feedback_enabled,
            tts_templates: cleanTemplates,
            bindings: payload.bindings !== undefined ? payload.bindings : current.bindings,
        };

        if (this.supabase) {
            try {
                // Upsert settings row
                const { error: settingsError } = await this.supabase
                    .from('guild_voice_settings')
                    .upsert({
                        guild_id: guildId,
                        enabled: updated.enabled,
                        wake_word: updated.wake_word,
                        language: updated.language,
                        tts_feedback_enabled: updated.tts_feedback_enabled,
                        tts_templates: {
                            ...cleanTemplates,
                            _voice: tts_voice,
                            _pitch: tts_pitch,
                            _rate: tts_rate,
                        },
                        updated_at: new Date().toISOString(),
                    });

                if (settingsError) {
                    log.warn({ err: settingsError.message, guildId }, 'Error updating voice settings in Supabase; cached in-memory');
                } else if (payload.bindings && Array.isArray(payload.bindings)) {
                    // If bindings were explicitly updated, sync bindings table
                    await this.supabase.from('guild_voice_bindings').delete().eq('guild_id', guildId);

                    if (payload.bindings.length > 0) {
                        const rows = payload.bindings.map((b) => ({
                            guild_id: guildId,
                            phrase: b.phrase.trim().toLowerCase(),
                            command_id: b.command_id.trim(),
                        }));
                        const { error: bindingsError } = await this.supabase
                            .from('guild_voice_bindings')
                            .insert(rows);
                        if (bindingsError) {
                            log.warn({ err: bindingsError.message, guildId }, 'Error updating voice bindings in Supabase');
                        }
                    }
                }
            } catch (err) {
                log.warn({ err: err.message, guildId }, 'Error updating voice settings in Supabase; cached in-memory');
            }
        }

        // Refresh cache
        this.cache.set(guildId, {
            data: updated,
            expiresAt: Date.now() + CACHE_TTL_MS,
        });

        return updated;
    }

    /**
     * Resets a guild's voice settings and custom bindings to defaults.
     *
     * @param {string} guildId
     * @returns {Promise<object>}
     */
    async resetGuildSettings(guildId) {
        if (this.supabase) {
            try {
                await this.supabase.from('guild_voice_bindings').delete().eq('guild_id', guildId);
                await this.supabase.from('guild_voice_settings').delete().eq('guild_id', guildId);
            } catch (err) {
                log.warn({ err: err.message, guildId }, 'Failed to delete settings from Supabase on reset');
            }
        }

        this.cache.delete(guildId);
        return this.getDefaultSettings();
    }

    /**
     * Clears local in-memory cache.
     */
    clearCache() {
        this.cache.clear();
    }
}

const voiceSettingsStore = new VoiceSettingsStore();

module.exports = {
    VoiceSettingsStore,
    voiceSettingsStore,
};
