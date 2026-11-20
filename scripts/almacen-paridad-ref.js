'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { JsonStore } = require('../src/core/store');

const defaultTargetDir = path.join(
    __dirname,
    '..',
    'rust',
    'crates',
    'nucleo',
    'tests',
    'data',
    'almacen',
);

async function escribir(targetDir) {
    fs.mkdirSync(targetDir, { recursive: true });
    const store = new JsonStore(targetDir);
    await store.initialize();

    await store.set('guild_simple', { language: 'es', volume: 80 });
    await store.set('guild_nested', {
        prefix: '!',
        filters: { bassboost: true, nightcore: false },
        users: ['123', '456'],
    });
    await store.set('special-key_123', {
        enabled: true,
        count: 42,
        name: 'test_store',
    });

    await store.drain();
    console.log(`Archivos de paridad generados en: ${targetDir}`);
}

async function leer(targetDir) {
    const store = new JsonStore(targetDir);
    await store.initialize();

    const keys = store.keys().sort();
    const values = {};
    for (const key of keys) {
        values[key] = store.get(key);
    }

    console.log(JSON.stringify({ keys, values }));
}

async function main() {
    const args = process.argv.slice(2);
    if (args[0] === '--leer') {
        const targetDir = args[1];
        if (!targetDir) {
            console.error('Error: se requiere la ruta para --leer');
            process.exit(1);
        }
        await leer(targetDir);
    } else {
        const targetDir = args[0] || defaultTargetDir;
        await escribir(targetDir);
    }
}

main().catch((err) => {
    console.error(err);
    process.exit(1);
});
