'use strict';

const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { pcmRms, slicePcmWav, downsample48kStereoTo16kMono } = require('../src/voice');

const FIXTURES_DIR = path.join(__dirname, '..', 'test', 'fixtures');
const DATA_DIR = path.join(__dirname, '..', 'rust', 'crates', 'voice', 'tests', 'data');
const OUT_FILE = path.join(DATA_DIR, 'audio-parity.json');

// Deterministic xorshift32 PRNG
let rngState = 0x12345678;
function nextUint32() {
    rngState ^= (rngState << 13) >>> 0;
    rngState ^= (rngState >>> 17) >>> 0;
    rngState ^= (rngState << 5) >>> 0;
    return rngState >>> 0;
}

function sha256(buf) {
    return crypto.createHash('sha256').update(buf).digest('hex');
}

function generateFixtureCases() {
    const files = fs.readdirSync(FIXTURES_DIR)
        .filter((f) => f.endsWith('.wav'))
        .sort();

    const results = [];
    const sliceOffsets = [0, 500, 1000, 1500];

    for (const fileName of files) {
        const fileBuf = fs.readFileSync(path.join(FIXTURES_DIR, fileName));
        const rawPcm = fileBuf.subarray(44);
        const rms = pcmRms(rawPcm);

        const slices = [];
        for (const startMs of sliceOffsets) {
            const res = slicePcmWav(rawPcm, startMs, 250);
            slices.push({
                startMs,
                preRollMs: 250,
                pcmSha256: sha256(res.pcm16k),
                wavSha256: sha256(res.wavBuffer),
                pcmSamples: res.pcm16k.length / 2,
                wavBytes: res.wavBuffer.length,
            });
        }

        results.push({
            file: fileName,
            rawPcmSha256: sha256(rawPcm),
            samples: rawPcm.length / 2,
            rms,
            slices,
        });
    }

    return results;
}

function generateDownsampleCases() {
    const cases = [];

    for (let i = 0; i < 1000; i++) {
        const sampleCount = nextUint32() % 200;
        const buf = Buffer.alloc(sampleCount * 2);

        for (let s = 0; s < sampleCount; s++) {
            const u16 = nextUint32() & 0xFFFF;
            const i16 = u16 > 0x7FFF ? u16 - 0x10000 : u16;
            buf.writeInt16LE(i16, s * 2);
        }

        const out = downsample48kStereoTo16kMono(buf);
        cases.push({
            index: i,
            inputSamples: sampleCount,
            outputSamples: out.length / 2,
            sha256: sha256(out),
        });
    }

    return cases;
}

function main() {
    fs.mkdirSync(DATA_DIR, { recursive: true });

    const fixtureCases = generateFixtureCases();
    const downsampleCases = generateDownsampleCases();

    const output = {
        prngSeed: '0x12345678',
        fixtures: fixtureCases,
        downsampleCases,
    };

    fs.writeFileSync(OUT_FILE, JSON.stringify(output, null, 2) + '\n', 'utf8');
    console.log(`Wrote ${fixtureCases.length} fixtures and ${downsampleCases.length} downsample cases to ${OUT_FILE}`);
}

main();
