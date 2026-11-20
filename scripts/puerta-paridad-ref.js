'use strict';

const fs = require('node:fs');
const path = require('node:path');

const { WakeGate } = require('../src/voice');

const FIXTURES_DIR = path.join(__dirname, '..', 'test', 'fixtures');
const DATA_DIR = path.join(__dirname, '..', 'rust', 'crates', 'voice', 'tests', 'data');
const OUT_FILE = path.join(DATA_DIR, 'puerta-casos.json');

const WAKE_PHRASES = ['ignite', 'bot', 'ignite bot'];

async function main() {
    fs.mkdirSync(DATA_DIR, { recursive: true });

    const gate = new WakeGate();
    const ready = await gate.initialize();
    if (!ready) {
        throw new Error('Failed to initialize WakeGate in Node');
    }

    const files = fs.readdirSync(FIXTURES_DIR)
        .filter((f) => f.endsWith('.wav'))
        .sort();

    const cases = [];

    for (const phrase of WAKE_PHRASES) {
        for (const file of files) {
            const pcm = fs.readFileSync(path.join(FIXTURES_DIR, file)).subarray(44);
            const res = gate.evaluate(pcm, phrase);
            cases.push({
                file,
                palabras: phrase,
                abierta: res.passed,
                motivo: res.reason,
                oido: res.heard,
                palabra: res.wakeWord || null,
                inicioMs: res.startMs ?? null,
                finMs: res.endMs ?? null,
                noSoportadas: res.unsupported || null,
            });
        }
    }

    gate.destroy();

    fs.writeFileSync(OUT_FILE, JSON.stringify(cases, null, 2) + '\n', 'utf8');
}

main().catch((err) => {
    console.error(err);
    process.exit(1);
});
