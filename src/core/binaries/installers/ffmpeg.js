'use strict';

const fs = require('node:fs');
const path = require('node:path');
const config = require('../../../config');
const log = require('../../logger').createLogger('setup');
const { probe } = require('../probe');

/**
 * ffmpeg: FFMPEG_PATH > ffmpeg-static (installed on demand) > ffmpeg on PATH.
 */
async function ensureFfmpeg() {
    if (config.ffmpegPath) {
        const result = await probe(config.ffmpegPath, ['-version']);
        if (!result.ok) log.error(`FFMPEG_PATH="${config.ffmpegPath}" does not work: ${result.error}`);
        return result.ok ? config.ffmpegPath : null;
    }

    let bundled = null;
    try {
        bundled = require('ffmpeg-static');
    } catch {
        // optional dependency not installed
    }
    if (bundled) {
        if (!fs.existsSync(bundled)) {
            // npm may have skipped ffmpeg-static's install script: run it now.
            log.info('Downloading ffmpeg...');
            const installer = path.join(path.dirname(require.resolve('ffmpeg-static/package.json')), 'install.js');
            const result = await probe(process.execPath, [installer], 10 * 60_000);
            if (!result.ok) log.warn(`Could not download ffmpeg: ${result.error}`);
        }
        if (fs.existsSync(bundled) && (await probe(bundled, ['-version'])).ok) return bundled;
    }

    if ((await probe('ffmpeg', ['-version'])).ok) return 'ffmpeg';
    log.error('No working ffmpeg found. Install ffmpeg or set FFMPEG_PATH.');
    return null;
}

module.exports = { ensureFfmpeg };
