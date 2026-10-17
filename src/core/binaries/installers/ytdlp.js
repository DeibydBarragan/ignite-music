'use strict';

const fs = require('node:fs');
const fsp = require('node:fs/promises');
const path = require('node:path');
const config = require('../../../config');
const log = require('../../logger').createLogger('setup');
const { probe } = require('../probe');
const { downloadToFile } = require('../downloader');
const { extractZip } = require('../../unzip');

const BIN_DIR = config.binDir;
const INSTALL_DIR = path.join(BIN_DIR, 'yt-dlp-dist');
const UPDATE_MARKER = path.join(config.dataDir, 'yt-dlp.checked');
const UPDATE_INTERVAL_MS = 24 * 3600_000;
const RELEASES = 'https://github.com/yt-dlp/yt-dlp/releases';
const MAX_ARCHIVE_BYTES = 256 * 1024 * 1024;
const MAX_EXTRACTED_BYTES = 1024 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES = 5000;
// Read at call time so a mirror configured later (tests) still takes effect.
const downloadBase = () => config.ytdlp.downloadBase || `${RELEASES}/latest/download`;

/**
 * Official yt-dlp build for this machine, as a ZIP of the program folder ("onedir").
 * These need no Python, and unlike the single-file builds they do not unpack themselves on
 * every run, which made each yt-dlp call take seconds (much longer on macOS).
 */
function ytDlpAsset(platform = process.platform, arch = process.arch, musl = isMusl()) {
    if (platform === 'win32') return { x64: 'yt-dlp_win.zip', ia32: 'yt-dlp_win_x86.zip', arm64: 'yt-dlp_win_arm64.zip' }[arch] || null;
    if (platform === 'darwin') return 'yt-dlp_macos.zip';
    if (platform === 'linux') {
        if (arch === 'x64') return musl ? 'yt-dlp_musllinux.zip' : 'yt-dlp_linux.zip';
        if (arch === 'arm64') return musl ? 'yt-dlp_musllinux_aarch64.zip' : 'yt-dlp_linux_aarch64.zip';
        if (arch === 'arm' && !musl) return 'yt-dlp_linux_armv7l.zip';
    }
    return null;
}

function isMusl() {
    if (process.platform !== 'linux') return false;
    try {
        const report = /** @type {any} */ (process.report.getReport());
        return !report.header.glibcVersionRuntime;
    } catch {
        return false;
    }
}

/**
 * Where the bot's own yt-dlp lives (YTDLP_PATH overrides it).
 */
function localYtDlpPath(platform = process.platform) {
    return path.join(INSTALL_DIR, platform === 'win32' ? 'yt-dlp.exe' : 'yt-dlp');
}

/**
 * Official checksum for an asset, when available. Custom mirrors
 * (YTDLP_DOWNLOAD_BASE) are not assumed to publish SHA2-256SUMS.
 */
async function assetChecksum(asset) {
    if (config.ytdlp.downloadBase) return null;
    try {
        const response = await fetch(`${downloadBase()}/SHA2-256SUMS`, { signal: AbortSignal.timeout(15_000) });
        if (!response.ok) return null;
        const line = (await response.text()).split(/\r?\n/).find((row) => row.trim().endsWith(asset));
        return line ? line.trim().split(/\s+/)[0] : null;
    } catch {
        return null;
    }
}

/**
 * Download and unpack yt-dlp into bin/yt-dlp-dist, replacing the previous version only once
 * the new one is known to run.
 */
async function installYtDlp(asset = ytDlpAsset()) {
    if (!asset) throw new Error(`No yt-dlp build for ${process.platform}/${process.arch}`);
    const archive = path.join(BIN_DIR, `.yt-dlp-${process.pid}.zip`);
    const staging = path.join(BIN_DIR, `.yt-dlp-${process.pid}`);
    await fsp.mkdir(BIN_DIR, { recursive: true });
    await fsp.rm(staging, { recursive: true, force: true });
    try {
        const checksum = await assetChecksum(asset);
        await downloadToFile(`${downloadBase()}/${asset}`, archive, {
            minBytes: 1024 * 1024,
            maxBytes: MAX_ARCHIVE_BYTES,
            sha256: checksum,
        });
        await extractZip(await fsp.readFile(archive), staging, {
            maxEntries: MAX_ARCHIVE_ENTRIES,
            maxBytes: MAX_EXTRACTED_BYTES,
        });
        const program = (await fsp.readdir(staging, { withFileTypes: true }))
            .find((entry) => entry.isFile() && entry.name.startsWith('yt-dlp'));
        if (!program) throw new Error('The yt-dlp archive does not contain the program');
        const executable = path.join(staging, path.basename(localYtDlpPath()));
        if (program.name !== path.basename(executable)) await fsp.rename(path.join(staging, program.name), executable);
        if (process.platform !== 'win32') await fsp.chmod(executable, 0o755);
        const check = await probe(executable);
        if (!check.ok) throw new Error(`the downloaded yt-dlp does not run: ${check.error}`);

        const previous = `${INSTALL_DIR}.old-${process.pid}`;
        if (fs.existsSync(INSTALL_DIR)) await fsp.rename(INSTALL_DIR, previous);
        await fsp.rename(staging, INSTALL_DIR);
        await fsp.rm(previous, { recursive: true, force: true });
        // Single-file builds used by earlier versions of the bot.
        for (const old of ['yt-dlp', 'yt-dlp.exe']) {
            const file = path.join(BIN_DIR, old);
            if (fs.existsSync(file) && (await fsp.stat(file)).isFile()) await fsp.rm(file, { force: true });
        }
        await markChecked();
        return check.output;
    } finally {
        await fsp.rm(staging, { recursive: true, force: true });
        await fsp.rm(archive, { force: true });
    }
}

/**
 * Make sure a working yt-dlp exists. Downloads it into bin/ when needed, so it works even when
 * npm skipped install scripts or the system Python is too old.
 */
async function ensureYtDlp() {
    if (config.ytdlp.path) {
        const result = await probe(config.ytdlp.path);
        if (!result.ok) log.error(`YTDLP_PATH="${config.ytdlp.path}" does not work: ${result.error}`);
        return result.ok ? config.ytdlp.path : null;
    }

    const local = localYtDlpPath();
    if (fs.existsSync(local)) {
        const result = await probe(local);
        if (result.ok) return local;
        log.warn(`bin/yt-dlp-dist does not run (${result.error}), downloading it again`);
    }

    if (ytDlpAsset()) {
        try {
            log.info(`Downloading yt-dlp (${ytDlpAsset()})...`);
            const version = await installYtDlp();
            log.ok(`yt-dlp ${version} installed`);
            return local;
        } catch (error) {
            log.error(`Could not install yt-dlp: ${error.message}`);
        }
    }

    const system = await probe('yt-dlp');
    if (system.ok) return 'yt-dlp';
    log.error('No working yt-dlp found. YouTube will not play. Download it from https://github.com/yt-dlp/yt-dlp/releases and set YTDLP_PATH.');
    return null;
}

async function markChecked() {
    await fsp.mkdir(config.dataDir, { recursive: true });
    await fsp.writeFile(UPDATE_MARKER, new Date().toISOString());
}

/**
 * Latest yt-dlp version, read from the redirect of the "latest release" page
 * (no GitHub API, so no API rate limit).
 */
async function latestYtDlpVersion() {
    const response = await fetch(`${RELEASES}/latest`, { method: 'HEAD', redirect: 'manual', signal: AbortSignal.timeout(15_000) });
    const tag = (response.headers.get('location') || '').match(/\/tag\/([^/?#]+)/)?.[1];
    if (!tag) throw new Error(`unexpected response (${response.status})`);
    return decodeURIComponent(tag);
}

/**
 * yt-dlp must stay current for YouTube to keep working: check once a day and install a newer
 * version before the shards start (nothing is running yt-dlp at that moment).
 */
async function updateYtDlp() {
    const local = localYtDlpPath();
    if (config.ytdlp.path || !fs.existsSync(local) || !ytDlpAsset()) return null;
    try {
        if (Date.now() - fs.statSync(UPDATE_MARKER).mtimeMs < UPDATE_INTERVAL_MS) return null;
    } catch {
        // never checked
    }
    try {
        const [current, latest] = await Promise.all([probe(local), latestYtDlpVersion()]);
        if (current.ok && current.output === latest) {
            await markChecked();
            return null;
        }
        log.info(`Updating yt-dlp ${current.output || ''} -> ${latest}...`);
        const version = await installYtDlp();
        log.ok(`yt-dlp ${version} installed`);
        return version;
    } catch (error) {
        log.warn(`yt-dlp update check failed (will retry on next start): ${error.message}`);
        return null;
    }
}

module.exports = {
    ytDlpAsset,
    localYtDlpPath,
    installYtDlp,
    ensureYtDlp,
    updateYtDlp,
    latestYtDlpVersion,
    INSTALL_DIR,
};
