'use strict';

/**
 * Audio transcoding utilities for voice recognition:
 * - Decodes Opus packets (Discord voice stream) into 48kHz 16-bit stereo PCM.
 * - Downsamples 48kHz stereo to 16kHz mono (optimal for STT / Whisper models).
 * - Wraps raw PCM in a standard RIFF/WAVE container.
 */

const prism = require('prism-media');

/**
 * Downsamples 48,000 Hz 16-bit stereo PCM to 16,000 Hz 16-bit mono PCM.
 * Ratio: 48000 / 16000 = 3:1.
 * Each input frame is 4 bytes (2 bytes L, 2 bytes R).
 *
 * @param {Buffer} pcm48k - Raw 48kHz 16-bit stereo PCM buffer.
 * @returns {Buffer} Raw 16kHz 16-bit mono PCM buffer.
 */
function downsample48kStereoTo16kMono(pcm48k) {
    if (!Buffer.isBuffer(pcm48k) || pcm48k.length < 12) {
        return Buffer.alloc(0);
    }

    const frameStep = 12; // 3 frames * 4 bytes
    const totalFrames = Math.floor(pcm48k.length / frameStep);
    const output = Buffer.alloc(totalFrames * 2);

    for (let i = 0; i < totalFrames; i++) {
        const offset = i * frameStep;
        const left = pcm48k.readInt16LE(offset);
        const right = pcm48k.readInt16LE(offset + 2);
        const monoSample = Math.round((left + right) / 2);
        output.writeInt16LE(monoSample, i * 2);
    }

    return output;
}

/**
 * Wraps raw PCM bytes in a 44-byte standard RIFF/WAVE header.
 *
 * @param {Buffer} pcmBuffer - Raw PCM data.
 * @param {number} [sampleRate=16000] - Audio sample rate.
 * @param {number} [numChannels=1] - Number of channels (1 = mono, 2 = stereo).
 * @param {number} [bitDepth=16] - Bits per sample.
 * @returns {Buffer} Formatted WAV file buffer.
 */
function pcmToWav(pcmBuffer, sampleRate = 16000, numChannels = 1, bitDepth = 16) {
    const dataSize = pcmBuffer.length;
    const header = Buffer.alloc(44);
    const bytesPerSample = bitDepth / 8;
    const blockAlign = numChannels * bytesPerSample;
    const byteRate = sampleRate * blockAlign;

    header.write('RIFF', 0);
    header.writeUInt32LE(36 + dataSize, 4);
    header.write('WAVE', 8);

    header.write('fmt ', 12);
    header.writeUInt32LE(16, 16); // Subchunk1Size for PCM
    header.writeUInt16LE(1, 20); // AudioFormat: 1 = PCM
    header.writeUInt16LE(numChannels, 22);
    header.writeUInt32LE(sampleRate, 24);
    header.writeUInt32LE(byteRate, 28);
    header.writeUInt16LE(blockAlign, 32);
    header.writeUInt16LE(bitDepth, 34);

    header.write('data', 36);
    header.writeUInt32LE(dataSize, 40);

    return Buffer.concat([header, pcmBuffer]);
}

/**
 * Consumes an Opus audio stream, decodes it to PCM, downsamples to 16kHz mono,
 * and outputs a WAV file buffer.
 *
 * @param {import('node:stream').Readable} opusStream - Readable Opus stream from Discord receiver.
 * @param {object} [options]
 * @param {number} [options.minDurationMs=400] - Minimum audio duration required (filters clicks).
 * @param {number} [options.maxDurationMs=12000] - Maximum duration allowed before aborting.
 * @returns {Promise<Buffer|null>} WAV Buffer, or null if audio is too short/silent.
 */
function opusStreamToWav(opusStream, { minDurationMs = 400, maxDurationMs = 12000 } = {}) {
    return new Promise((resolve, reject) => {
        const decoder = new prism.opus.Decoder({ rate: 48000, channels: 2, frameSize: 960 });
        const pcmChunks = [];
        let totalBytes = 0;
        let finished = false;

        // Max byte limit (48kHz * 4 bytes per frame * (maxDurationMs / 1000))
        const maxBytes = Math.floor(48000 * 4 * (maxDurationMs / 1000));
        // Min byte limit
        const minBytes = Math.floor(48000 * 4 * (minDurationMs / 1000));

        const cleanup = () => {
            finished = true;
            try { opusStream.destroy(); } catch { /* ignore */ }
            try { decoder.destroy(); } catch { /* ignore */ }
        };

        const timeout = setTimeout(() => {
            if (!finished) {
                cleanup();
                onStreamEnd();
            }
        }, maxDurationMs + 1000);

        function onStreamEnd() {
            if (finished && pcmChunks.length === 0) return;
            finished = true;
            clearTimeout(timeout);

            if (totalBytes < minBytes) {
                return resolve(null);
            }

            try {
                const pcm48k = Buffer.concat(pcmChunks, totalBytes);
                const pcm16k = downsample48kStereoTo16kMono(pcm48k);
                if (pcm16k.length === 0) {
                    return resolve(null);
                }
                const wav = pcmToWav(pcm16k, 16000, 1, 16);
                resolve(wav);
            } catch (err) {
                reject(err);
            }
        }

        decoder.on('data', (chunk) => {
            if (finished) return;
            pcmChunks.push(chunk);
            totalBytes += chunk.length;
            if (totalBytes >= maxBytes) {
                cleanup();
                onStreamEnd();
            }
        });

        decoder.once('end', onStreamEnd);
        decoder.once('error', (err) => {
            cleanup();
            clearTimeout(timeout);
            reject(err);
        });

        opusStream.once('error', (err) => {
            cleanup();
            clearTimeout(timeout);
            reject(err);
        });

        opusStream.pipe(decoder);
    });
}

module.exports = {
    downsample48kStereoTo16kMono,
    pcmToWav,
    opusStreamToWav,
};
