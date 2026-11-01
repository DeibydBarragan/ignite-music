'use strict';

/**
 * Audio transcoding utilities for voice recognition:
 * - Decodes Opus packets (Discord voice stream) into 48kHz 16-bit stereo PCM.
 * - Downsamples 48kHz stereo to 16kHz mono (optimal for STT / Whisper models).
 * - Wraps raw PCM in a standard RIFF/WAVE container.
 */

const prism = require('prism-media');

/**
 * Resilient Opus Decoder that gracefully drops corrupted, padded, or malformed
 * Opus frames without aborting or destroying the audio stream.
 */
class SafeOpusDecoder extends prism.opus.Decoder {
    _transform(chunk, encoding, done) {
        try {
            super._transform(chunk, encoding, (err) => {
                if (err) {
                    // Suppress frame decode error (e.g. corrupted packet) and continue streaming
                    return done();
                }
                return done();
            });
        } catch {
            return done();
        }
    }
}

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
        const decoder = new SafeOpusDecoder({ rate: 48000, channels: 2, frameSize: 960 });
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
            // If we already accumulated sufficient audio, salvage it instead of rejecting
            if (totalBytes >= minBytes) {
                cleanup();
                onStreamEnd();
            } else {
                cleanup();
                clearTimeout(timeout);
                reject(err);
            }
        });

        opusStream.once('error', (err) => {
            if (totalBytes >= minBytes) {
                cleanup();
                onStreamEnd();
            } else {
                cleanup();
                clearTimeout(timeout);
                reject(err);
            }
        });

        opusStream.pipe(decoder);
    });
}

/**
 * Root-mean-square level of 16-bit mono PCM (0 = silence, 32768 = full scale).
 *
 * @param {Buffer} pcm16k - Raw 16-bit little-endian mono PCM.
 * @returns {number}
 */

/**
 * Trims leading audio from a 16kHz mono 16-bit PCM buffer up to a given start time,
 * preserving a pre-roll buffer so the beginning of the wake word is not clipped.
 * Returns the sliced PCM and its formatted WAV container.
 *
 * @param {Buffer} pcm16k - 16kHz mono 16-bit PCM buffer.
 * @param {number} [startMs=0] - Millisecond offset where speech of interest begins.
 * @param {number} [preRollMs=250] - Extra audio before startMs to preserve acoustic onset.
 * @returns {{ pcm16k: Buffer, wavBuffer: Buffer }}
 */
function slicePcmWav(pcm16k, startMs = 0, preRollMs = 250) {
    if (!Buffer.isBuffer(pcm16k) || pcm16k.length === 0) {
        const empty = Buffer.alloc(0);
        return { pcm16k: empty, wavBuffer: pcmToWav(empty) };
    }

    if (!startMs || startMs <= 0) {
        return { pcm16k, wavBuffer: pcmToWav(pcm16k) };
    }

    const effectiveStartMs = Math.max(0, startMs - preRollMs);
    // 16kHz * 2 bytes/sample / 1000 ms = 32 bytes per ms
    const byteOffset = Math.floor(effectiveStartMs * 32);
    // Ensure 2-byte alignment for 16-bit PCM samples
    const alignedOffset = byteOffset - (byteOffset % 2);

    // Keep at least 400ms (12,800 bytes) of tail audio; otherwise don't over-slice
    if (alignedOffset >= pcm16k.length - 12800) {
        return { pcm16k, wavBuffer: pcmToWav(pcm16k) };
    }

    const slicedPcm = pcm16k.subarray(alignedOffset);
    return {
        pcm16k: slicedPcm,
        wavBuffer: pcmToWav(slicedPcm),
    };
}

function pcmRms(pcm16k) {
    const samples = Math.floor((pcm16k?.length || 0) / 2);
    if (samples === 0) return 0;
    let sumSquares = 0;
    for (let i = 0; i < samples; i++) {
        const s = pcm16k.readInt16LE(i * 2);
        sumSquares += s * s;
    }
    return Math.sqrt(sumSquares / samples);
}

module.exports = {
    downsample48kStereoTo16kMono,
    pcmToWav,
    opusStreamToWav,
    pcmRms,
    slicePcmWav,
};
