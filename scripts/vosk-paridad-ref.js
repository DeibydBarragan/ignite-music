'use strict';

const fs = require('node:fs');
const path = require('node:path');

// Ensure unit test context so the English model is used (per S-05)
process.env.NODE_TEST_CONTEXT = '1';

const { VoskModelManager } = require('../src/voice');

const FIXTURES_DIR = path.join(__dirname, '..', 'test', 'fixtures');
const DATA_DIR = path.join(__dirname, '..', 'rust', 'crates', 'voice', 'tests', 'data');
const OUT_FILE = path.join(DATA_DIR, 'vosk-texto.json');

async function main() {
    fs.mkdirSync(DATA_DIR, { recursive: true });

    const manager = new VoskModelManager();
    const ready = await manager.initialize();
    if (!ready) {
        throw new Error('Failed to initialize VoskModelManager in Node');
    }

    const rec = manager.acquireRecognizer(null);
    if (!rec) {
        throw new Error('Failed to acquire unconstrained recognizer');
    }

    const files = fs.readdirSync(FIXTURES_DIR)
        .filter((f) => f.endsWith('.wav'))
        .sort();

    const results = {};
    for (const fileName of files) {
        const filePath = path.join(FIXTURES_DIR, fileName);
        const wavBuffer = fs.readFileSync(filePath);
        const pcm16k = wavBuffer.subarray(44);
        const { text } = manager.processUtterance(rec, pcm16k);
        results[fileName] = text;
    }

    manager.releaseRecognizer(rec);
    manager.destroy();

    fs.writeFileSync(OUT_FILE, JSON.stringify(results, null, 2) + '\n', 'utf8');
}

main().catch((err) => {
    console.error(err);
    process.exit(1);
});
