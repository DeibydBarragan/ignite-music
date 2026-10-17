'use strict';

const fs = require('node:fs');
const fsp = require('node:fs/promises');
const path = require('node:path');
const config = require('../../../config');
const log = require('../../logger').createLogger('setup');
const { probe } = require('../probe');
const { downloadToFile } = require('../downloader');

const BIN_DIR = config.binDir;
const DOWNLOAD_BASE = 'https://github.com/cloudflare/cloudflared/releases/latest/download';

function cloudflaredAsset(platform = process.platform, arch = process.arch) {
    if (platform === 'win32') {
        return arch === 'ia32' ? 'cloudflared-windows-386.exe' : 'cloudflared-windows-amd64.exe';
    }
    if (platform === 'linux') {
        if (arch === 'arm64') return 'cloudflared-linux-arm64';
        if (arch === 'arm') return 'cloudflared-linux-arm';
        return 'cloudflared-linux-amd64';
    }
    if (platform === 'darwin') {
        return arch === 'arm64' ? 'cloudflared-darwin-arm64' : 'cloudflared-darwin-amd64';
    }
    return null;
}

function localCloudflaredPath(platform = process.platform) {
    return path.join(BIN_DIR, platform === 'win32' ? 'cloudflared.exe' : 'cloudflared');
}

/**
 * Cloudflare Tunnel: checks local bin, root, and PATH.
 * If missing or corrupted (e.g. EFTYPE / bad format), automatically downloads the official release.
 */
async function ensureCloudflared() {
    const local = localCloudflaredPath();
    const commonPaths = process.platform === 'win32'
        ? [
            local,
            path.join(config.root, 'cloudflared.exe'),
            'C:\\Program Files (x86)\\cloudflared\\cloudflared.exe',
            'C:\\Program Files\\cloudflared\\cloudflared.exe',
        ]
        : [local, path.join(config.root, 'cloudflared')];

    // Check existing candidates and verify executable integrity
    for (const candidate of commonPaths) {
        if (fs.existsSync(candidate)) {
            const check = await probe(candidate, ['--version']);
            if (check.ok) return candidate;
            log.warn(`Existing cloudflared at ${candidate} is corrupted/invalid (${check.error}), replacing with official build...`);
            await fsp.rm(candidate, { force: true }).catch(() => {});
        }
    }

    // Check system PATH
    const system = await probe('cloudflared', ['--version']);
    if (system.ok) return 'cloudflared';

    // Download official binary for current OS and architecture
    const asset = cloudflaredAsset();
    if (!asset) {
        log.warn(`No official cloudflared binary available for ${process.platform}/${process.arch}.`);
        return null;
    }

    log.info(`Downloading official cloudflared for ${process.platform}/${process.arch}...`);
    const tempPath = path.join(BIN_DIR, `.cloudflared-${process.pid}.tmp`);
    try {
        await fsp.mkdir(BIN_DIR, { recursive: true });
        await downloadToFile(`${DOWNLOAD_BASE}/${asset}`, tempPath, {
            minBytes: 10 * 1024 * 1024,
            maxBytes: 128 * 1024 * 1024,
        });
        if (process.platform !== 'win32') {
            await fsp.chmod(tempPath, 0o755);
        }

        const verify = await probe(tempPath, ['--version']);
        if (!verify.ok) throw new Error(`Downloaded cloudflared failed verification: ${verify.error}`);

        await fsp.rm(local, { force: true });
        await fsp.rename(tempPath, local);
        log.ok(`cloudflared installed successfully (${verify.output.split(/\r?\n/)[0]})`);
        return local;
    } catch (error) {
        await fsp.rm(tempPath, { force: true }).catch(() => {});
        log.warn(`Failed to auto-download cloudflared: ${error.message}`);
        return null;
    }
}

module.exports = { ensureCloudflared, cloudflaredAsset, localCloudflaredPath };
