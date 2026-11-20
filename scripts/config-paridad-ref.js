'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const root = path.join(__dirname, '..');
const casosDir = path.join(root, 'rust', 'crates', 'nucleo', 'tests', 'data', 'env-casos');

const childScript = `
const Module = require('module');
const dotenv = require('dotenv');
// Neutralize dotenv.config() so the reference is generated without the real .env.
const __fake = Object.assign({}, dotenv, { config: () => ({ parsed: {} }) });
const _load = Module._load;
Module._load = function (request, parent, isMain) {
    if (request === 'dotenv') return __fake;
    return _load.apply(this, arguments);
};
const envFile = process.argv[1];
const content = require('node:fs').readFileSync(envFile, 'utf8');
const parsed = dotenv.parse(content);
for (const [k, v] of Object.entries(parsed)) {
    process.env[k] = v;
}
const cfg = require('./src/config');
const out = {
    debug: cfg.debug,
    noColor: cfg.noColor,
    shardId: cfg.shardId,
    devUserIds: cfg.devUserIds,
    discord: {
        token: cfg.discord.token,
        clientId: cfg.discord.clientId,
        guildId: cfg.discord.guildId,
    },
    bot: {
        status: cfg.bot.status,
        color: cfg.bot.color,
        supportServer: cfg.bot.supportServer,
        website: cfg.bot.website,
        defaultLanguage: cfg.bot.defaultLanguage,
        defaultVolume: cfg.bot.defaultVolume,
        maxVolume: cfg.bot.maxVolume,
        maxQueueSize: cfg.bot.maxQueueSize,
        maxPlaylistSize: cfg.bot.maxPlaylistSize,
        leaveOnEmptyMs: cfg.bot.leaveOnEmptyMs,
        leaveOnFinishMs: cfg.bot.leaveOnFinishMs,
        panelRefreshMs: cfg.bot.panelRefreshMs,
        invite: cfg.bot.invite,
    },
    spotify: {
        clientId: cfg.spotify.clientId,
        clientSecret: cfg.spotify.clientSecret,
    },
    supabase: {
        url: cfg.supabase.url,
        key: cfg.supabase.key,
        serviceRoleKey: cfg.supabase.serviceRoleKey,
    },
    ytdlp: {
        path: cfg.ytdlp.path,
        cookiesFile: cfg.ytdlp.cookiesFile,
        cookiesFromBrowser: cfg.ytdlp.cookiesFromBrowser,
        poToken: cfg.ytdlp.poToken,
        potGenerator: cfg.ytdlp.potGenerator,
        potGeneratorPath: cfg.ytdlp.potGeneratorPath,
        extractorArgs: cfg.ytdlp.extractorArgs,
        concurrency: cfg.ytdlp.concurrency,
        timeoutMs: cfg.ytdlp.timeoutMs,
        downloadBase: cfg.ytdlp.downloadBase,
    },
    innertube: {
        hl: cfg.innertube.hl,
        gl: cfg.innertube.gl,
    },
    ffmpegPath: cfg.ffmpegPath,
    proxies: cfg.proxies,
    sharding: {
        totalShards: String(cfg.sharding.totalShards),
        shardList: cfg.sharding.shardList,
        mode: cfg.sharding.mode,
        managed: cfg.sharding.managed,
        respawn: cfg.sharding.respawn,
        spawnDelay: cfg.sharding.spawnDelay,
        spawnTimeout: cfg.sharding.spawnTimeout,
    },
    api: {
        enabled: cfg.api.enabled,
        port: cfg.api.port,
        secret: cfg.api.secret,
        allowedOrigins: cfg.api.allowedOrigins,
        rateLimitPerMinute: cfg.api.rateLimitPerMinute,
        publicUrl: cfg.api.publicUrl,
        autoTunnel: cfg.api.autoTunnel,
    },
    voice: {
        enabled: cfg.voice.enabled,
        defaultWakeWord: cfg.voice.defaultWakeWord,
        groqApiKey: cfg.voice.groqApiKey,
        silenceDurationMs: cfg.voice.silenceDurationMs,
        model: cfg.voice.model,
        prompt: cfg.voice.prompt,
        offlineGating: cfg.voice.offlineGating,
        minRms: cfg.voice.minRms,
        groqCooldownMs: cfg.voice.groqCooldownMs,
        soundApiUrl: cfg.voice.soundApiUrl,
        soundApiSecret: cfg.voice.soundApiSecret,
        ttsFeedbackEnabled: cfg.voice.ttsFeedbackEnabled,
        allowBotInput: cfg.voice.allowBotInput,
        soundBotId: cfg.voice.soundBotId,
    },
};
process.stdout.write(JSON.stringify(out, null, 2));
`;

if (!fs.existsSync(casosDir)) {
    console.error(`Casos dir does not exist: ${casosDir}`);
    process.exit(1);
}

const files = fs.readdirSync(casosDir).filter((f) => f.endsWith('.env'));
for (const file of files) {
    const fullPath = path.join(casosDir, file);
    const jsonPath = fullPath.replace(/\.env$/, '.json');
    // Run child node process with clean env to prevent outer env leakage
    const cleanEnv = {
        PATH: process.env.PATH,
        SYSTEMROOT: process.env.SYSTEMROOT,
        COMSPEC: process.env.COMSPEC,
        PATHEXT: process.env.PATHEXT,
    };
    const out = execFileSync(process.execPath, ['-e', childScript, fullPath], {
        cwd: root,
        env: cleanEnv,
        encoding: 'utf8',
    });
    fs.writeFileSync(jsonPath, out + '\n');
    console.log(`Generated parity reference for: ${file} -> ${path.basename(jsonPath)}`);
}
