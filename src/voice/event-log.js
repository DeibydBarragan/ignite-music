'use strict';

/**
 * In-memory ring buffer for live voice interaction logs per guild.
 * Used for Developer Mode voice debugging and live console monitoring.
 */

const MAX_LOGS_PER_GUILD = 100;

const { voiceCsvExporter } = require('./csv-exporter');

class VoiceEventLogStore {
    constructor(maxPerGuild = MAX_LOGS_PER_GUILD, csvExporter = voiceCsvExporter) {
        this.maxPerGuild = maxPerGuild;
        this.csvExporter = csvExporter;
        this.logs = new Map(); // guildId -> VoiceLogEntry[]
    }

    /**
     * Records a voice interaction entry.
     *
     * @param {object} entry
     * @returns {object} The normalized entry with id and timestamp.
     */
    record(entry) {
        const guildId = String(entry.guildId);
        if (!this.logs.has(guildId)) {
            this.logs.set(guildId, []);
        }

        const logEntry = {
            id: `vlog_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
            timestamp: new Date().toISOString(),
            guildId,
            userId: entry.userId ? String(entry.userId) : null,
            audioBytes: Number(entry.audioBytes) || 0,
            voskHeard: entry.voskHeard || '',
            wakeWordSpotted: Boolean(entry.wakeWordSpotted),
            matchedWakeWord: entry.matchedWakeWord || null,
            gateReason: entry.gateReason || null,
            groqEngaged: Boolean(entry.groqEngaged),
            groqText: entry.groqText || null,
            groqElapsedMs: entry.groqElapsedMs || null,
            commandId: entry.commandId || null,
            query: entry.query || null,
            ttsMessage: entry.ttsMessage || null,
            status: entry.status || 'filtered_out',
        };

        const list = this.logs.get(guildId);
        list.push(logEntry);
        if (list.length > this.maxPerGuild) {
            list.shift();
        }

        // Volcado a CSV: cada 50 registros nuevos de este guild se escribe un archivo.
        if (this.csvExporter) {
            try {
                this.csvExporter.add(logEntry);
            } catch {
                // El volcado a disco nunca debe tumbar el registro en memoria.
            }
        }

        // Broadcast to SSE if sseHub is active
        try {
            const { getSseHub } = require('../api/sse');
            const hub = getSseHub();
            if (hub) {
                hub.publish(guildId, 'voice.log', logEntry);
            }
        } catch {
            // Ignore broadcast failure if server is not yet listening
        }

        return logEntry;
    }

    /**
     * Retrieves recent logs for a guild (newest first).
     *
     * @param {string} guildId
     * @param {number} [limit=50]
     * @returns {object[]}
     */
    getLogs(guildId, limit = 50) {
        const list = this.logs.get(String(guildId)) || [];
        return list.slice(-limit).reverse();
    }

    /**
     * Clears all recorded logs for a guild.
     *
     * @param {string} guildId
     */
    clear(guildId) {
        this.logs.delete(String(guildId));
    }
}

const voiceEventLogStore = new VoiceEventLogStore();

module.exports = {
    VoiceEventLogStore,
    voiceEventLogStore,
};
