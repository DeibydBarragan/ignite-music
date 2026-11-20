'use strict';

const fs = require('node:fs');
const path = require('node:path');
const cp = require('node:child_process');
const crypto = require('node:crypto');
const { defaultFilters } = require('distube');
const { FILTERS, FILTER_LABELS } = require('../src/music/filters');

const targetPath = path.join(
    __dirname,
    '..',
    'rust',
    'crates',
    'player',
    'tests',
    'data',
    'filtros-casos.json',
);

const fixturePath = path.join(__dirname, '..', 'test', 'fixtures', 'ignite-sample.wav');

function findFfmpeg() {
    if (process.env.FFMPEG_PATH) return process.env.FFMPEG_PATH;
    try {
        const staticPath = require('ffmpeg-static');
        if (staticPath && fs.existsSync(staticPath)) return staticPath;
    } catch {
        // fallback
    }
    const binName = process.platform === 'win32' ? 'ffmpeg.exe' : 'ffmpeg';
    const binPath = path.join(__dirname, '..', 'bin', binName);
    if (fs.existsSync(binPath)) return binPath;
    return 'ffmpeg';
}

const ffmpegBin = findFfmpeg();

const cases = FILTERS.map((name) => {
    const filterArg = defaultFilters[name] || name;
    let audioSha256 = null;
    let audioBytes = 0;

    try {
        const args = ['-i', fixturePath, '-af', filterArg, '-f', 's16le', '-ac', '2', '-ar', '48000', 'pipe:1'];
        const res = cp.spawnSync(ffmpegBin, args, { maxBuffer: 10 * 1024 * 1024 });
        if (res.status === 0 && res.stdout && res.stdout.length > 0) {
            audioSha256 = crypto.createHash('sha256').update(res.stdout).digest('hex');
            audioBytes = res.stdout.length;
        }
    } catch {
        // fallback if ffmpeg is unavailable
    }

    return {
        name,
        label: FILTER_LABELS[name] || name,
        ffmpeg_arg: filterArg,
        audio_sha256: audioSha256,
        audio_bytes: audioBytes,
    };
});

const output = {
    fixture: 'ignite-sample.wav',
    filters: cases,
    order: FILTERS,
};

fs.mkdirSync(path.dirname(targetPath), { recursive: true });
fs.writeFileSync(targetPath, `${JSON.stringify(output, null, 2)}\n`, 'utf8');
console.log(`Generated ${cases.length} filter cases into ${targetPath}`);
