
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { VoiceCsvExporter, buildCsv, toCsvCell } = require('../src/voice/csv-exporter');
const { VoiceEventLogStore } = require('../src/voice/event-log');

test('csv: toCsvCell escapa comas, comillas y saltos de linea', () => {
    assert.equal(toCsvCell('hola'), 'hola');
    assert.equal(toCsvCell('a,b'), '"a,b"');
    assert.equal(toCsvCell('di "hola"'), '"di ""hola"""');
    assert.equal(toCsvCell('a\nb'), '"a\nb"');
    assert.equal(toCsvCell(null), '');
    assert.equal(toCsvCell(0), '0');
});

test('csv: buildCsv genera cabecera y una fila por entrada', () => {
    const csv = buildCsv([
        { id: '1', guildId: 'g', voskHeard: 'ignite pausa', groqEngaged: true },
    ]);
    const lines = csv.trim().split('\n');
    assert.equal(lines.length, 2);
    assert.match(lines[0], /^id,timestamp,guildId/);
    assert.match(lines[1], /^1,/);
});

test('csv: el exporter vuelca un archivo cada 50 registros', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'voice-csv-'));
    try {
        const exporter = new VoiceCsvExporter({ dir, batchSize: 50 });

        // 49 registros: aun no se escribe nada
        for (let i = 0; i < 49; i++) {
            assert.equal(exporter.add({ guildId: 'g1', id: `e${i}` }), null);
        }
        assert.equal(fs.readdirSync(dir).length, 0);

        // El registro 50 dispara el volcado
        const file = exporter.add({ guildId: 'g1', id: 'e49' });
        assert.ok(file, 'debe devolver la ruta del CSV escrito');
        const written = fs.readdirSync(dir).filter((f) => f.endsWith('.csv'));
        assert.equal(written.length, 1);

        const content = fs.readFileSync(path.join(dir, written[0]), 'utf8').trim().split('\n');
        assert.equal(content.length, 51); // cabecera + 50 filas

        // Los siguientes 50 generan un segundo archivo
        for (let i = 50; i < 100; i++) exporter.add({ guildId: 'g1', id: `e${i}` });
        assert.equal(fs.readdirSync(dir).filter((f) => f.endsWith('.csv')).length, 2);

        // Otro guild lleva su propio lote independiente
        for (let i = 0; i < 49; i++) exporter.add({ guildId: 'g2', id: `x${i}` });
        assert.equal(fs.readdirSync(dir).filter((f) => f.endsWith('.csv')).length, 2);
    } finally {
        fs.rmSync(dir, { recursive: true, force: true });
    }
});

test('csv: event-log dispara el volcado a traves del exporter inyectado', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'voice-csv-el-'));
    try {
        const exporter = new VoiceCsvExporter({ dir, batchSize: 50 });
        const store = new VoiceEventLogStore(100, exporter);
        for (let i = 0; i < 50; i++) {
            store.record({ guildId: 'g1', voskHeard: `frase ${i}`, groqEngaged: false });
        }
        const written = fs.readdirSync(dir).filter((f) => f.endsWith('.csv'));
        assert.equal(written.length, 1);
        const rows = fs.readFileSync(path.join(dir, written[0]), 'utf8').trim().split('\n');
        assert.equal(rows.length, 51);
    } finally {
        fs.rmSync(dir, { recursive: true, force: true });
    }
});

test('csv: flushAll vuelca los lotes parciales de todos los guilds', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'voice-csv-flush-'));
    try {
        const exporter = new VoiceCsvExporter({ dir, batchSize: 50 });
        for (let i = 0; i < 10; i++) exporter.add({ guildId: 'g1', id: `a${i}` });
        for (let i = 0; i < 3; i++) exporter.add({ guildId: 'g2', id: `b${i}` });
        // Aun no hay archivos: ningun lote llego a 50
        assert.equal(fs.readdirSync(dir).filter((f) => f.endsWith('.csv')).length, 0);

        const files = exporter.flushAll();
        assert.equal(files.length, 2); // un parcial por guild
        assert.equal(fs.readdirSync(dir).filter((f) => f.endsWith('.csv')).length, 2);

        // Un segundo flushAll no escribe nada: ya no queda pendiente
        assert.equal(exporter.flushAll().length, 0);
    } finally {
        fs.rmSync(dir, { recursive: true, force: true });
    }
});
