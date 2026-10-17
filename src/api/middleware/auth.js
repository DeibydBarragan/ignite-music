'use strict';

const crypto = require('node:crypto');
const config = require('../../config');
const logger = require('../../core/logger');
const { sendProblem } = require('../contracts');

const log = logger.createLogger('api-auth');

// preflight.js refuses to boot when the API is enabled with a weaker secret;
// this module also fails closed so a request can never slip through.
const MIN_SECRET_LENGTH = 32;
const SNOWFLAKE = /^\d{17,20}$/;
const MUTATION_METHODS = new Set(['POST', 'PUT', 'PATCH', 'DELETE']);

/**
 * Extract the token from an "Authorization: Bearer <token>" header.
 * Returns null when the header is missing or uses another scheme.
 */
function extractBearer(header) {
    if (typeof header !== 'string') return null;
    const match = /^Bearer\s+(\S+)$/i.exec(header.trim());
    return match ? match[1] : null;
}

/**
 * Constant-time string comparison that never throws on length mismatch.
 */
function safeEqual(a, b) {
    if (typeof a !== 'string' || typeof b !== 'string') return false;
    const left = Buffer.from(a, 'utf8');
    const right = Buffer.from(b, 'utf8');
    if (left.length !== right.length) return false;
    return crypto.timingSafeEqual(left, right);
}

/**
 * Bearer authentication. Unlike the previous implementation there is no
 * bypass when the secret is empty: weak/missing configuration means 503.
 */
function requireAuth(req, res, next) {
    const secret = config.api.secret || '';
    if (secret.length < MIN_SECRET_LENGTH) {
        log.error('API_SECRET is missing or too short; refusing every API request.');
        return sendProblem(res, 503, 'AUTH_NOT_CONFIGURED', 'API authentication is not configured.');
    }

    const token = extractBearer(req.headers.authorization);
    if (!token || !safeEqual(token, secret)) {
        return sendProblem(res, 401, 'UNAUTHORIZED', 'Invalid or missing bearer token.');
    }
    return next();
}

/**
 * State-changing requests must identify the acting Discord user. The Next.js
 * BFF sends it as `X-User-Discord-Id`; it is validated as a snowflake and
 * exposed to routes as `req.discordUserId`.
 */
function requireUser(req, res, next) {
    if (!MUTATION_METHODS.has(req.method)) return next();

    const raw = req.headers['x-user-discord-id'];
    const value = typeof raw === 'string' ? raw.trim() : '';
    if (!SNOWFLAKE.test(value)) {
        return sendProblem(res, 400, 'INVALID_USER', 'Missing or invalid X-User-Discord-Id header (Discord snowflake required).');
    }

    req.discordUserId = value;
    return next();
}

module.exports = { requireAuth, requireUser, extractBearer, safeEqual, MIN_SECRET_LENGTH, SNOWFLAKE };
