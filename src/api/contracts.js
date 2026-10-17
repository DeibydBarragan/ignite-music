'use strict';

/**
 * Single source of truth for what the REST API puts on the wire:
 * - serializeTrack(): DisTube Song -> frontend Song contract (replaces the
 *   duplicated formatSong in server.js and toFrontendTrack in playlists.js),
 *   with a stable in-memory `trackId` for queue operations.
 * - RFC 7807 problem documents for every error response. The legacy `error`
 *   extension key repeats `detail`: the Next.js BFF and lib/http.ts read it.
 */

const crypto = require('node:crypto');
const { isLyricSong } = require('../music/plugins');
const { FALLBACK_COVER } = require('../core/constants');
const { resolveRequester, requesterName } = require('../core/user');

const ERROR_BASE = 'https://ignite.bot/errors';

const ERROR_TITLES = {
    400: 'Invalid Request Parameter',
    401: 'Unauthorized',
    403: 'Forbidden',
    404: 'Not Found',
    409: 'Conflict',
    413: 'Payload Too Large',
    422: 'Unprocessable Entity',
    429: 'Too Many Requests',
    500: 'Internal Server Error',
    502: 'Bad Gateway',
    503: 'Service Unavailable',
};

function getAvatarUrl(obj) {
    if (!obj) return null;
    for (const method of ['displayAvatarURL', 'avatarURL']) {
        if (typeof obj[method] !== 'function') continue;
        try {
            const url = obj[method]();
            if (url) return url;
        } catch {
            // Fall through to plain avatar fields.
        }
    }
    if (typeof obj.avatar === 'string' && obj.avatar.startsWith('http')) return obj.avatar;
    return null;
}

function formatDuration(seconds) {
    const total = Number(seconds) || 0;
    return `${Math.floor(total / 60)}:${String(Math.floor(total % 60)).padStart(2, '0')}`;
}

/**
 * Stable identity for a queue element (assigned once per Song instance).
 */
function ensureTrackId(song) {
    if (!song) return null;
    if (!song.trackId) song.trackId = crypto.randomUUID();
    return song.trackId;
}

/**
 * DisTube Song -> frontend Song (same shape as the old formatSong).
 * `extended: false` omits queue-only fields (trackId, requestedBy, isAutoplay)
 * for persisted snapshots such as user_playlists.tracks.
 */
function serializeTrack(song, { extended = true } = {}) {
    if (!song) return null;
    const user = resolveRequester(song);
    const base = {
        id: song.id || song.url,
        title: song.name || 'Unknown Title',
        artist: song.uploader?.name || 'Unknown Artist',
        albumArt: song.thumbnail || FALLBACK_COVER,
        coverUrl: song.thumbnail || null,
        duration: song.duration || 0,
        formattedDuration: song.formattedDuration && song.formattedDuration !== '00:00'
            ? song.formattedDuration
            : (song.duration ? formatDuration(song.duration) : '00:00'),
        // The dashboard schema only understands youtube/spotify.
        source: song.source === 'spotify' ? 'spotify' : 'youtube',
        url: song.url,
        category: 'hits',
        isLive: Boolean(song.isLive),
        isLyric: isLyricSong(song.url),
    };
    if (!extended) return base;
    return {
        trackId: ensureTrackId(song),
        ...base,
        isAutoplay: Boolean(song.autoplay || !user || user.bot),
        requestedBy: user ? {
            id: user.id,
            username: requesterName(song, user),
            avatar: getAvatarUrl(song.member) || getAvatarUrl(user) || (typeof user.avatar === 'string' ? user.avatar : null),
        } : null,
    };
}

/**
 * Operational error thrown by API operations; the shard proxy transports
 * status/code/message and routes render them as RFC 7807 documents.
 */
class ApiError extends Error {
    /**
     * @param {number} status
     * @param {string} code
     * @param {string} detail
     * @param {{ title?: string, type?: string }} [options]
     */
    constructor(status, code, detail, { title, type } = {}) {
        super(detail);
        this.name = 'ApiError';
        this.status = status;
        this.code = code;
        this.title = title || ERROR_TITLES[status] || 'Error';
        this.type = type || `${ERROR_BASE}/${String(code).toLowerCase().replace(/_/g, '-')}`;
    }
}

function problem(status, code, detail, options = {}) {
    const { title, type, ...fields } = options;
    return {
        type: type || `${ERROR_BASE}/${String(code).toLowerCase().replace(/_/g, '-')}`,
        title: title || ERROR_TITLES[status] || 'Error',
        status,
        detail,
        code,
        timestamp: new Date().toISOString(),
        // Legacy extension key: the BFF and lib/http.ts show this string.
        error: detail,
        ...fields,
    };
}

function sendProblem(res, status, code, detail, options = {}) {
    return res.status(status).type('application/problem+json').json(problem(status, code, detail, options));
}

/**
 * Render any thrown value as a problem document.
 */
function sendError(res, error) {
    if (error instanceof ApiError) {
        return sendProblem(res, error.status, error.code, error.message, { title: error.title, type: error.type });
    }
    return sendProblem(res, 500, 'INTERNAL_ERROR', error?.message || 'Internal server error');
}

// Transport-level failures from the shard proxy have no ApiError status.
const STATUS_BY_CODE = {
    guild_not_found: 404,
    shard_timeout: 503,
    not_ready: 503,
    unknown_op: 500,
    op_failed: 500,
};

/**
 * Express handler result: JSON body when the op succeeded, RFC 7807 problem
 * otherwise. Ops throw ApiError so `status` travels through the shard proxy.
 */
function sendOpResult(res, result) {
    if (result && result.ok) return res.json(result.data);
    const code = result?.code || 'op_failed';
    const status = Number(result?.status) || STATUS_BY_CODE[code] || 500;
    return sendProblem(res, status, code, result?.message || 'Operation failed');
}

module.exports = {
    FALLBACK_COVER,
    ERROR_TITLES,
    ApiError,
    serializeTrack,
    ensureTrackId,
    formatDuration,
    getAvatarUrl,
    problem,
    sendProblem,
    sendError,
    sendOpResult,
    STATUS_BY_CODE,
};
