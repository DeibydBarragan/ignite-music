'use strict';

/**
 * Second startup step (npm packages are installed by now): configuration, Discord token,
 * yt-dlp, ffmpeg and voice libraries. Fixes what it can and explains everything else.
 */
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const config = require('../config');
const binaries = require('../core/binaries');
const supabaseModule = require('../core/supabase');
const pot = require('../music/pot');
const report = require('./report');
const {
    MIN_API_SECRET_LENGTH,
    SNOWFLAKE,
    TOKEN,
    BROWSERS,
    TOKEN_STEPS,
    installHelp,
    ensureEnvFile,
    validateProxy,
    checkToken,
    canWrite,
    loads,
} = require('./preflight-checks');

async function run({ fetchImpl = fetch } = {}) {
    let problems = 0;
    const fail = (label, lines) => {
        problems++;
        report.fail(label, lines);
    };

    // ─── .env and Discord ──────────────────────────────────────────────
    if (ensureEnvFile()) {
        fail('A .env file was created for you, but it still needs your bot token', TOKEN_STEPS);
        report.stop(problems);
    }

    const { token } = config.discord;
    if (!token) {
        fail('DISCORD_TOKEN is empty', TOKEN_STEPS);
    } else if (!TOKEN.test(token)) {
        fail('DISCORD_TOKEN does not look like a bot token', [
            'Copy the token again without spaces or quotes. It looks like: MTA1...xyz.Gh7k2a.abc...',
            ...TOKEN_STEPS,
        ]);
    } else {
        const result = await checkToken(token, fetchImpl);
        if (result.ok) {
            const app = result.application;
            const name = app.bot?.username ? `${app.bot.username}` : app.name;
            if (config.discord.clientId && config.discord.clientId !== app.id) {
                report.warn(`CLIENT_ID in .env (${config.discord.clientId}) belongs to another application`, [
                    `Using this bot's own id instead: ${app.id}. You can remove CLIENT_ID from .env.`,
                ]);
            }
            // config.discord.clientId is a live getter over CLIENT_ID, so this
            // single write serves the manager, deploy and every shard.
            process.env.CLIENT_ID = app.id;
            report.ok('Discord token', `${name} (application ${app.id})`);
        } else if (result.reason === 'invalid') {
            fail('Discord rejected DISCORD_TOKEN (wrong or reset token)', TOKEN_STEPS);
        } else {
            if (!config.discord.clientId) {
                fail('Could not reach Discord to check the token', [
                    `Reason: ${result.detail}`,
                    'Check the internet connection of this machine (and firewall / proxy settings).',
                ]);
            } else {
                report.warn('Could not reach Discord to check the token, trying anyway', [`Reason: ${result.detail}`]);
            }
        }
    }

    if (config.discord.guildId && !SNOWFLAKE.test(config.discord.guildId)) {
        fail(`GUILD_ID "${config.discord.guildId}" is not a server id`, [
            'Leave GUILD_ID empty to register commands everywhere, or put a server id (Developer Mode > right click the server > Copy Server ID).',
        ]);
    }

    // ─── Settings ──────────────────────────────────────────────────────
    for (const proxy of config.proxies) {
        if (!validateProxy(proxy)) {
            fail(`PROXY_URL "${proxy}" is not a valid proxy address`, ['Use the form http://user:password@host:port (several can be separated by commas).']);
        }
    }
    if (config.ytdlp.cookiesFile) {
        const file = path.resolve(config.root, config.ytdlp.cookiesFile);
        if (!fs.existsSync(file)) {
            fail(`COOKIES_FILE "${config.ytdlp.cookiesFile}" does not exist`, [
                `Expected at: ${file}`,
                'Export your YouTube cookies to that file (see README.md, "YouTube: Sign in to confirm you\'re not a bot"),',
                'or remove COOKIES_FILE from .env.',
            ]);
        } else {
            // config.ytdlp.cookiesFile is a live getter over COOKIES_FILE, so
            // the resolved absolute path flows to every shard from here.
            process.env.COOKIES_FILE = file;
            report.ok('YouTube cookies', file);
        }
    } else if (config.ytdlp.cookiesFromBrowser) {
        const browser = config.ytdlp.cookiesFromBrowser.split(':')[0].toLowerCase();
        if (!BROWSERS.includes(browser)) fail(`COOKIES_FROM_BROWSER "${config.ytdlp.cookiesFromBrowser}" is not a supported browser`, [`Use one of: ${BROWSERS.join(', ')}`]);
        else report.ok('YouTube cookies', `from ${browser}`);
    }
    if (!canWrite(config.dataDir)) {
        fail(`Cannot write to ${config.dataDir}`, ['The bot saves settings there. Give your user write permission to the bot folder.']);
    }

    // ─── API security ──────────────────────────────────────────────────
    if (config.api.enabled) {
        if ((config.api.secret || '').length < MIN_API_SECRET_LENGTH) {
            fail(`API_SECRET is missing or too short (minimum ${MIN_API_SECRET_LENGTH} characters)`, [
                'The REST API refuses to start with a weak, empty or default secret.',
                'Generate one and add it to .env:',
                '  node -e "console.log(require(\'crypto\').randomBytes(32).toString(\'hex\'))"',
                'Or set API_ENABLED=false to run the bot without the web dashboard API.',
            ]);
        } else {
            report.ok('API secret', `configured (${MIN_API_SECRET_LENGTH}+ characters)`);
        }
        if (config.api.allowedOrigins.length === 0) {
            report.info('API_ALLOWED_ORIGINS is empty', 'browser cross-origin requests will be rejected (server-to-server clients are unaffected)');
        }
    }

    // ─── Audio tools ───────────────────────────────────────────────────
    const [ytdlpPath, ffmpegPath] = await Promise.all([
        // A daily update check runs here, before the shards start using yt-dlp.
        binaries.ensureYtDlp().then(async (found) => {
            if (found) await binaries.updateYtDlp();
            return found;
        }),
        binaries.ensureFfmpeg(),
    ]);
    if (ytdlpPath) {
        report.ok('yt-dlp', (await binaries.probe(ytdlpPath)).output);
    } else {
        fail('yt-dlp is missing (needed for YouTube)', [
            'The automatic download failed. Check the internet connection and start again, or install it yourself:',
            installHelp('yt-dlp'),
            'and set YTDLP_PATH in .env if it is not on your PATH.',
        ]);
    }
    if (ffmpegPath) {
        const version = (await binaries.probe(ffmpegPath, ['-version'])).output?.split(/\r?\n/)[0].replace(/ Copyright.*$/, '');
        report.ok('ffmpeg', version);
    } else {
        fail('ffmpeg is missing (needed to play any audio)', [
            'The automatic download failed. Check the internet connection and start again, or install it yourself:',
            installHelp('ffmpeg'),
            'and set FFMPEG_PATH in .env if it is not on your PATH.',
        ]);
    }

    // Mint the PO token while the shards boot, so the first downloads carry it.
    pot.startPoTokenRenewal();

    const voiceProblems = [
        !loads('@snazzah/davey') && '@snazzah/davey (Discord voice encryption)',
        !loads('@discordjs/opus') && '@discordjs/opus (audio encoder)',
        !crypto.getCiphers().includes('aes-256-gcm') && 'aes-256-gcm support in Node.js',
    ].filter(Boolean);
    if (voiceProblems.length) {
        fail('Voice libraries do not load', [
            `Broken: ${voiceProblems.join(', ')}`,
            'Usually node_modules was copied from another computer. Delete the node_modules folder and start the bot again.',
        ]);
    } else {
        report.ok('Voice libraries', 'opus, encryption, DAVE');
    }

    // ─── Optional ──────────────────────────────────────────────────────
    if (!config.spotify.clientId || !config.spotify.clientSecret) {
        report.info('Spotify API keys not set', 'optional: large Spotify playlists load only their first 100 songs');
    }

    if (supabaseModule.supabase) {
        const health = await supabaseModule.verifyConnection();
        if (health.ok) report.ok('Supabase', 'connection verified');
        else report.warn('Supabase connection could not be verified', [
            `Reason: ${health.reason}`,
            'History, favorites and web playlists degrade until this is fixed.',
        ]);
    } else {
        report.info('Supabase not configured', 'optional: play history, favorites and web playlists are disabled');
    }

    if (problems) report.stop(problems);
    console.log('');
    // Failures throw SetupError via report.stop; reaching this line always means success.
    return { ok: true, problems: [] };
}

module.exports = { run, ensureEnvFile, validateProxy, checkToken, TOKEN };
