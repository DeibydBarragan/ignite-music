'use strict';

const fs = require('node:fs');
const path = require('node:path');

const { buildCsv, toCsvCell } = require('../src/voice/csv-exporter');

const DATA_DIR = path.join(__dirname, '..', 'rust', 'crates', 'voice', 'tests', 'data');
const OUT_FILE = path.join(DATA_DIR, 'csv-casos.json');

const ENTRIES = [
    {
        id: 'vlog_1731000000000_abc12',
        timestamp: '2026-11-08T22:00:00.000Z',
        guildId: '123456789',
        userId: '987654321',
        audioBytes: 32000,
        voskHeard: 'ignite reproduce starboy',
        wakeWordSpotted: true,
        matchedWakeWord: 'ignite',
        gateReason: 'ok',
        groqEngaged: true,
        groqText: 'ignite reproduce starboy',
        groqElapsedMs: 342,
        commandId: 'play',
        query: 'starboy',
        ttsMessage: 'Reproduciendo starboy',
        status: 'executed',
    },
    {
        id: 'vlog_1731000001000_def34',
        timestamp: '2026-11-08T22:00:01.000Z',
        guildId: '123456789',
        userId: null,
        audioBytes: 0,
        voskHeard: '',
        wakeWordSpotted: false,
        matchedWakeWord: null,
        gateReason: 'gate_unavailable',
        groqEngaged: false,
        groqText: null,
        groqElapsedMs: null,
        commandId: null,
        query: null,
        ttsMessage: null,
        status: 'filtered_out',
    },
    {
        id: 'vlog_1731000002000_ghi56',
        timestamp: '2026-11-08T22:00:02.000Z',
        guildId: '999',
        userId: '888',
        audioBytes: 16000,
        voskHeard: 'di "hola", que tal',
        wakeWordSpotted: true,
        matchedWakeWord: 'ignite',
        gateReason: 'ok',
        groqEngaged: true,
        groqText: 'linea 1\nlinea 2\r\nlinea 3 con, comas y "comillas"',
        groqElapsedMs: 1200,
        commandId: 'play',
        query: 'cancion, con coma y "comillas"',
        ttsMessage: 'Mensaje con\nsaltos de linea',
        status: 'executed',
    },
];

const CELL_CASES = [
    { input: 'hola', expected: toCsvCell('hola') },
    { input: 'a,b', expected: toCsvCell('a,b') },
    { input: 'di "hola"', expected: toCsvCell('di "hola"') },
    { input: 'a\nb', expected: toCsvCell('a\nb') },
    { input: 'a\r\nb', expected: toCsvCell('a\r\nb') },
    { input: null, expected: toCsvCell(null) },
    { input: undefined, expected: toCsvCell(undefined) },
    { input: 0, expected: toCsvCell(0) },
    { input: 12345, expected: toCsvCell(12345) },
    { input: true, expected: toCsvCell(true) },
    { input: false, expected: toCsvCell(false) },
    { input: '', expected: toCsvCell('') },
];

function main() {
    fs.mkdirSync(DATA_DIR, { recursive: true });

    const csvContent = buildCsv(ENTRIES);

    const payload = {
        cellCases: CELL_CASES,
        entries: ENTRIES,
        expectedCsv: csvContent,
    };

    fs.writeFileSync(OUT_FILE, JSON.stringify(payload, null, 2) + '\n', 'utf8');
}

main();
