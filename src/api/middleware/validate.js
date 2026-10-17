'use strict';

/**
 * Zod validation at the API boundary. Invalid input never reaches an
 * operation: it is rejected with a 400 RFC 7807 problem document.
 *
 * POST /play also guards against SSRF: yt-dlp would fetch any URL a caller
 * sends, so local/private targets (localhost, 127.0.0.0/8, 169.254.0.0/16,
 * RFC1918 ranges, file:// etc.) are refused while plain search text passes.
 */

const { z } = require('zod');
const { sendProblem } = require('../contracts');

const SNOWFLAKE_PATTERN = /^\d{17,20}$/;
const TRACK_ID_PATTERN = /^[A-Za-z0-9_-]{8,64}$/;

const snowflake = z.string().regex(SNOWFLAKE_PATTERN, 'must be a Discord snowflake (17-20 digits)');

// Query/body fields where legacy clients send "", null or nothing at all.
const optionalSnowflake = z.union([z.literal(''), z.null(), snowflake]).optional();

const trackId = z.string().regex(TRACK_ID_PATTERN, 'must be a valid track id');

const controlValue = z.union([z.string().max(200), z.number(), z.boolean(), z.null()]);

const schemas = {
    params: {
        guildId: z.object({ guildId: snowflake }),
        guildAndIndex: z.object({ guildId: snowflake, index: z.coerce.number().int().min(1) }),
        guildAndTrack: z.object({ guildId: snowflake, trackId }),
    },
    body: {
        play: z.object({
            query: z.string().trim().min(1, 'is required').max(1000),
            voiceChannelId: optionalSnowflake,
            userId: optionalSnowflake,
            next: z.boolean().optional(),
            skip: z.boolean().optional(),
        }),
        control: z.object({
            action: z.string().trim().min(1).max(32),
            value: controlValue.optional(),
        }),
        move: z.object({
            from: z.coerce.number().int().min(1),
            to: z.coerce.number().int().min(1),
        }),
        jump: z.object({
            position: z.coerce.number().int().min(1),
        }),
        order: z.object({
            trackId,
            to: z.coerce.number().int().min(1),
        }),
    },
    query: {
        voiceStatus: z.object({ userId: optionalSnowflake }),
        search: z.object({
            q: z.string().trim().max(500).optional(),
            limit: z.coerce.number().int().min(1).max(15).optional(),
        }),
        suggestions: z.object({ q: z.string().trim().max(500).optional() }),
        topSongs: z.object({
            timeframe: z.enum(['week', 'month', 'all']).optional(),
            limit: z.coerce.number().int().min(1).max(100).optional(),
        }),
        resolvePlaylist: z.object({ url: z.string().trim().min(1).max(1000) }),
    },
};

const BLOCKED_SCHEMES = /^(file|data|javascript|vbscript|blob):/i;
const EXPLICIT_SCHEME = /^[a-z][a-z0-9+.-]*:\/\//i;
const PRIVATE_HOST_SUFFIXES = ['.localhost', '.local', '.internal', '.home.arpa'];
const PRIVATE_HOSTS = new Set(['localhost', 'localhost.localdomain', 'metadata.google.internal']);

// WHATWG URL serializes IPv4-mapped IPv6 as hex ("[::ffff:7f00:1]").
function decodeMappedIpv4(mapped) {
    const match = /^([0-9a-f]{1,4}):([0-9a-f]{1,4})$/.exec(mapped);
    if (!match) return null;
    const hi = parseInt(match[1], 16);
    const lo = parseInt(match[2], 16);
    return [hi >> 8, hi & 0xff, lo >> 8, lo & 0xff].join('.');
}

function isPrivateIpv4(address) {
    const parts = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(address);
    if (!parts) return false;
    const [a, b] = [Number(parts[1]), Number(parts[2])];
    if (a === 0 || a === 10 || a === 127) return true;
    if (a === 172 && b >= 16 && b <= 31) return true;
    if (a === 192 && b === 168) return true;
    if (a === 169 && b === 254) return true; // link local + cloud metadata (169.254.169.254)
    return false;
}

function isPrivateHostname(hostname) {
    const host = String(hostname || '').toLowerCase();
    if (!host) return false;
    if (PRIVATE_HOSTS.has(host) || PRIVATE_HOST_SUFFIXES.some((suffix) => host.endsWith(suffix))) return true;

    const raw = host.startsWith('[') && host.endsWith(']') ? host.slice(1, -1) : host;
    if (raw === '::1' || raw === '::') return true;
    if (/^f[cd][0-9a-f]{2}:/.test(raw)) return true; // fc00::/7 unique local
    if (/^fe[89ab][0-9a-f]:/.test(raw)) return true; // fe80::/10 link local

    const mapped = raw.startsWith('::ffff:') ? raw.slice(7) : raw;
    return isPrivateIpv4(mapped) || isPrivateIpv4(decodeMappedIpv4(mapped) || '');
}

/**
 * Inspect a /play (or playlist resolve) target. Returns null when it is safe:
 * a plain search term, or an http(s) URL that is not local/private.
 * Otherwise returns { code, detail }.
 */
function inspectTarget(rawQuery) {
    const value = String(rawQuery || '').trim();
    if (!value) return null;
    if (BLOCKED_SCHEMES.test(value)) {
        return { code: 'BLOCKED_URL', detail: 'Local file and script URLs are not allowed.' };
    }
    if (!EXPLICIT_SCHEME.test(value)) return null; // search text ("daft punk one more time")

    let parsed;
    try {
        parsed = new URL(value);
    } catch {
        return null;
    }
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
        return { code: 'BLOCKED_URL', detail: `URL scheme "${parsed.protocol}" is not allowed.` };
    }
    if (isPrivateHostname(parsed.hostname)) {
        return { code: 'BLOCKED_URL', detail: 'URLs pointing to local or private networks are not allowed.' };
    }
    return null;
}

function firstIssue(error) {
    const issue = error.issues?.[0];
    if (!issue) return 'invalid value';
    const path = issue.path.length ? `${issue.path.join('.')}: ` : '';
    return `${path}${issue.message}`;
}

function validate(schema, source, code) {
    return (req, res, next) => {
        const parsed = schema.safeParse(req[source]);
        if (!parsed.success) {
            return sendProblem(res, 400, code, firstIssue(parsed.error));
        }
        req.validated ??= {};
        req.validated[source] = parsed.data;
        return next();
    };
}

const validateParams = (schema, code = 'INVALID_PARAMETER') => validate(schema, 'params', code);
const validateBody = (schema, code = 'INVALID_PARAMETER') => validate(schema, 'body', code);
const validateQuery = (schema, code = 'INVALID_PARAMETER') => validate(schema, 'query', code);

const snowflakeParams = (schema = schemas.params.guildId) => validateParams(schema, 'INVALID_SNOWFLAKE');

module.exports = {
    z,
    snowflake,
    SNOWFLAKE_PATTERN,
    schemas,
    validateParams,
    validateBody,
    validateQuery,
    snowflakeParams,
    inspectTarget,
    isPrivateHostname,
};
