'use strict';

const path = require('node:path');

require('dotenv').config({ path: path.join(__dirname, '..', '.env'), quiet: true });

const env = (key, fallback = '') => {
    const value = process.env[key];
    return value === undefined || value.trim() === '' ? fallback : value.trim();
};

const int = (key, fallback, { min = -Infinity, max = Infinity } = {}) => {
    const parsed = Number.parseInt(env(key), 10);
    if (!Number.isFinite(parsed)) return fallback;
    return Math.min(max, Math.max(min, parsed));
};

const bool = (key, fallback = false) => {
    const value = env(key).toLowerCase();
    if (!value) return fallback;
    return ['1', 'true', 'yes', 'on'].includes(value);
};

const list = (key) => env(key).split(',').map((item) => item.trim()).filter(Boolean);

const parseColor = (value) => {
    const hex = String(value).replace('#', '');
    const parsed = Number.parseInt(hex, 16);
    return /^[0-9a-f]{6}$/i.test(hex) && Number.isFinite(parsed) ? parsed : 0x5865f2;
};

const shardCount = () => {
    const value = env('TOTAL_SHARDS', 'auto');
    if (value === 'auto') return 'auto';
    const parsed = Number.parseInt(value, 10);
    return Number.isInteger(parsed) && parsed > 0 ? parsed : 'auto';
};

const shardList = () => {
    const value = env('SHARD_LIST', 'auto');
    if (value === 'auto') return 'auto';
    const ids = value.split(',').map((id) => Number.parseInt(id, 10)).filter(Number.isInteger);
    return ids.length ? ids : 'auto';
};

const dnsServers = list('DNS_SERVERS');

const config = {
    root: path.join(__dirname, '..'),
    dataDir: path.resolve(path.join(__dirname, '..'), env('DATA_DIR', 'data')),
    binDir: path.resolve(path.join(__dirname, '..'), env('BIN_DIR', 'bin')),
    debug: bool('DEBUG'),
    noColor: Boolean(env('NO_COLOR')),
    shardId: env('SHARDS') || null,

    discord: {
        token: env('DISCORD_TOKEN'),
        // Live getter: preflight.js discovers the application id from the token
        // and publishes it through process.env so the manager, deploy and every
        // shard see it without mutating this (frozen) object.
        get clientId() {
            return env('CLIENT_ID');
        },
        guildId: env('GUILD_ID') || null,
    },

    bot: {
        status: env('STATUS', '/play'),
        color: parseColor(env('EMBED_COLOR', '#5865F2')),
        supportServer: env('SUPPORT_SERVER'),
        website: env('WEBSITE'),
        defaultLanguage: env('DEFAULT_LANGUAGE', 'en'),
        defaultVolume: int('DEFAULT_VOLUME', 100, { min: 1, max: 100 }),
        maxVolume: 100,
        maxQueueSize: int('MAX_QUEUE_SIZE', 500, { min: 1 }),
        maxPlaylistSize: int('MAX_PLAYLIST_SIZE', 200, { min: 1 }),
        leaveOnEmptyMs: int('LEAVE_ON_EMPTY', 60, { min: 0 }) * 1000,
        leaveOnFinishMs: int('LEAVE_ON_FINISH', 120, { min: 0 }) * 1000,
        panelRefreshMs: int('PANEL_REFRESH', 20, { min: 0 }) * 1000,
        // Built from the live client id above, so it appears as soon as
        // preflight discovers the application id from the token.
        get invite() {
            const clientId = config.discord.clientId;
            return clientId
                ? `https://discord.com/oauth2/authorize?client_id=${clientId}&permissions=3230720&scope=bot%20applications.commands`
                : '';
        },
    },

    spotify: {
        clientId: env('SPOTIFY_CLIENT_ID'),
        clientSecret: env('SPOTIFY_CLIENT_SECRET'),
    },

    supabase: {
        url: env('SUPABASE_URL'),
        key: env('SUPABASE_KEY') || env('SUPABASE_ANON_KEY'),
        serviceRoleKey: env('SUPABASE_SERVICE_ROLE_KEY'),
    },

    ytdlp: {
        path: env('YTDLP_PATH'),
        // Live getter: preflight.js resolves the configured file and publishes
        // the absolute path through process.env for every shard.
        get cookiesFile() {
            return env('COOKIES_FILE');
        },
        cookiesFromBrowser: env('COOKIES_FROM_BROWSER'),
        poToken: env('YOUTUBE_PO_TOKEN'),
        potGenerator: env('POT_GENERATOR'),
        potGeneratorPath: env('POT_GENERATOR_PATH'),
        extractorArgs: env('YTDLP_EXTRACTOR_ARGS'),
        concurrency: int('YTDLP_CONCURRENCY', 4, { min: 1, max: 32 }),
        timeoutMs: int('YTDLP_TIMEOUT', 30, { min: 5 }) * 1000,
        // Live getter: tests point downloads at a local server by setting the
        // variable after this module was loaded.
        get downloadBase() {
            return env('YTDLP_DOWNLOAD_BASE');
        },
    },

    innertube: {
        // Language/region sent to YouTube's Innertube API (search, next, browse).
        hl: env('INNERTUBE_HL', 'es'),
        gl: env('INNERTUBE_GL', 'CO'),
    },

    ffmpegPath: env('FFMPEG_PATH'),
    proxies: list('PROXY_URL'),

    dns: {
        // Public resolvers used when the system resolver is slow or broken (see src/core/dns-fix.js).
        servers: dnsServers.length ? dnsServers : ['1.1.1.1', '8.8.8.8'],
        lookupTimeoutMs: int('DNS_LOOKUP_TIMEOUT_MS', 1500, { min: 100 }),
    },

    sharding: {
        totalShards: shardCount(),
        shardList: shardList(),
        mode: env('SHARD_MODE', 'process') === 'worker' ? 'worker' : 'process',
        // True inside a shard process (the manager publishes it to the shard
        // environment); false in the manager and in standalone runs.
        managed: bool('SHARDING_MANAGER'),
        respawn: bool('SHARD_RESPAWN', true),
        spawnDelay: int('SHARD_SPAWN_DELAY', 5500, { min: 0 }),
        spawnTimeout: int('SHARD_SPAWN_TIMEOUT', 60000, { min: 5000 }),
    },

    api: {
        enabled: bool('API_ENABLED', true),
        port: int('API_PORT', 3001, { min: 1024, max: 65535 }),
        // No default on purpose: preflight.js refuses to start the bot while the
        // API is enabled and the secret is missing or shorter than 32 chars.
        secret: env('API_SECRET'),
        // Browser origins allowed to call the API. Empty = no browser
        // cross-origin requests (server-to-server clients are unaffected).
        allowedOrigins: list('API_ALLOWED_ORIGINS'),
        rateLimitPerMinute: int('API_RATE_LIMIT_PER_MINUTE', 120, { min: 1, max: 100000 }),
        // Public URL of this bot's API (tunnel, API Gateway...). Published to
        // Supabase so the dashboard discovers it at runtime. Empty = disabled.
        publicUrl: env('PUBLIC_API_URL'),
        // Automatically start cloudflared quick tunnel and announce URL to Supabase
        autoTunnel: bool('AUTO_TUNNEL', false),
    },
};

config.parseColor = parseColor;

/**
 * Recursively freeze the configuration so runtime code cannot drift from
 * .env: startup fixes (preflight) publish changes through process.env and
 * the live getters above, never by assigning into this object.
 */
const deepFreeze = (value) => {
    if (!value || typeof value !== 'object' || Object.isFrozen(value)) return value;
    Object.freeze(value);
    for (const descriptor of Object.values(Object.getOwnPropertyDescriptors(value))) {
        if ('value' in descriptor) deepFreeze(descriptor.value);
    }
    return value;
};

module.exports = deepFreeze(config);
