'use strict';

/**
 * Persistencia a CSV de los logs de voz: por cada guild, cada vez que se acumulan
 * CSV_BATCH_SIZE registros nuevos, se vuelca ese lote a un archivo .csv bajo
 * data/voice-logs/. Es independiente del ring buffer en memoria (event-log.js),
 * que sigue sirviendo a la consola en vivo.
 */

const fs = require('fs');
const path = require('path');
const config = require('../config');
const log = require('../core/logger').createLogger('voice-csv');

const CSV_BATCH_SIZE = 50;

// Columnas del CSV, en orden. Coinciden con los campos de la entrada normalizada.
const CSV_COLUMNS = [
    'id',
    'timestamp',
    'guildId',
    'userId',
    'audioBytes',
    'voskHeard',
    'wakeWordSpotted',
    'matchedWakeWord',
    'gateReason',
    'groqEngaged',
    'groqText',
    'groqElapsedMs',
    'commandId',
    'query',
    'ttsMessage',
    'status',
];

/** Escapa un valor para una celda CSV (comillas dobles, comas, saltos de línea). */
function toCsvCell(value) {
    if (value === null || value === undefined) return '';
    const str = String(value);
    if (/[",\n\r]/.test(str)) {
        return `"${str.replace(/"/g, '""')}"`;
    }
    return str;
}

/** Construye el contenido CSV (con cabecera) de un lote de entradas. */
function buildCsv(entries) {
    const header = CSV_COLUMNS.join(',');
    const rows = entries.map((e) => CSV_COLUMNS.map((col) => toCsvCell(e[col])).join(','));
    return `${header}\n${rows.join('\n')}\n`;
}

class VoiceCsvExporter {
    /**
     * @param {object} [options]
     * @param {string} [options.dir] - Carpeta destino. Por defecto data/voice-logs.
     * @param {number} [options.batchSize] - Cuántos registros por archivo.
     */
    constructor(options = {}) {
        this.dir = options.dir || path.join(config.dataDir, 'voice-logs');
        this.batchSize = options.batchSize || CSV_BATCH_SIZE;
        this.pending = new Map(); // guildId -> entry[] aún no volcadas
    }

    /**
     * Añade una entrada al lote del guild y, al completar batchSize, escribe el CSV.
     *
     * @param {object} entry - Entrada normalizada de event-log.
     * @returns {string|null} Ruta del archivo escrito, o null si aún no se completó el lote.
     */
    add(entry) {
        const guildId = String(entry.guildId);
        if (!this.pending.has(guildId)) this.pending.set(guildId, []);
        const batch = this.pending.get(guildId);
        batch.push(entry);

        if (batch.length < this.batchSize) return null;

        const toFlush = batch.splice(0, this.batchSize);
        return this.flush(guildId, toFlush);
    }

    /**
     * Escribe un lote a disco. Separado para poder forzar un volcado parcial.
     *
     * @param {string} guildId
     * @param {object[]} entries
     * @returns {string|null} Ruta escrita, o null si no había nada o falló.
     */
    flush(guildId, entries) {
        if (!entries || entries.length === 0) return null;
        try {
            fs.mkdirSync(this.dir, { recursive: true });
            const stamp = new Date().toISOString().replace(/[:.]/g, '-');
            const file = path.join(this.dir, `${guildId}-${stamp}.csv`);
            fs.writeFileSync(file, buildCsv(entries), 'utf8');
            log.info({ guildId, count: entries.length, file }, 'Lote de logs de voz volcado a CSV');
            return file;
        } catch (err) {
            log.warn({ err, guildId }, 'No se pudo escribir el CSV de logs de voz');
            return null;
        }
    }

    /**
     * Vuelca a disco TODOS los lotes parciales pendientes (los que aún no llegaron
     * a batchSize). Se llama al apagar el bot para no perder los últimos registros.
     *
     * @returns {string[]} Rutas de los archivos escritos.
     */
    flushAll() {
        const written = [];
        for (const [guildId, batch] of this.pending.entries()) {
            if (batch.length === 0) continue;
            const file = this.flush(guildId, batch.splice(0));
            if (file) written.push(file);
        }
        return written;
    }
}

const voiceCsvExporter = new VoiceCsvExporter();

module.exports = {
    VoiceCsvExporter,
    voiceCsvExporter,
    buildCsv,
    toCsvCell,
    CSV_BATCH_SIZE,
    CSV_COLUMNS,
};
