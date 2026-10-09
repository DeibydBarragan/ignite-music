'use strict';

// Prints a fresh YouTube PO token (or NONE). Used for manual player_client
// experiments on the server without touching the bot.
// Usage: POT=$(node scripts/mint-pot.js)
const fs = require('node:fs');
const { spawn } = require('node:child_process');

const CANDIDATES = [
    process.env.POT_GENERATOR,
    '/home/ubuntu/bgutil-ytdlp-pot-provider/server/build/generate_once.js',
].filter(Boolean);

const generator = CANDIDATES.find((p) => {
    try {
        return fs.existsSync(p);
    } catch {
        return false;
    }
});

if (!generator) {
    console.log('NONE');
    process.exit(0);
}

let stdout = '';
const child = spawn(process.execPath, [generator], {
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
});

const timer = setTimeout(() => {
    try {
        child.kill('SIGKILL');
    } catch {}
}, 180_000);
if (timer.unref) timer.unref();

child.stdout.on('data', (chunk) => {
    stdout += chunk.toString('utf8');
});
child.on('close', () => {
    clearTimeout(timer);
    try {
        const start = stdout.indexOf('{');
        const parsed = JSON.parse(start >= 0 ? stdout.slice(start) : stdout);
        if (parsed && typeof parsed.poToken === 'string' && parsed.poToken.length > 10) {
            console.log(parsed.poToken);
            process.exit(0);
        }
    } catch {}
    console.log('NONE');
});
