'use strict';

/**
 * Internal REST API. This module only wires the HTTP shell (security
 * middleware, rate limiting, health) and mounts the v1 router; routes live in
 * src/api/v1/*.routes.js and execute domain operations through the
 * ShardProxy + MusicManager.apiOps (see src/api/shard-proxy.js).
 */

const crypto = require('node:crypto');
const express = require('express');
const cors = require('cors');
const helmet = require('helmet');
const { rateLimit } = require('express-rate-limit');
const config = require('../config');
const { createRouter } = require('./v1/router');
const { sendError } = require('./contracts');
const { getSseHub } = require('./sse');
const logger = require('../core/logger');

const log = logger.createLogger('api');

/**
 * Start the internal REST API server attached to the Discord client.
 */
function startApiServer(client) {
    if (!config.api.enabled) {
        log.info('API server is disabled in config.');
        return null;
    }

    const app = express();
    app.disable('x-powered-by');
    app.use(helmet());

    // CORS: only the browser origins explicitly listed in API_ALLOWED_ORIGINS.
    // Requests without an Origin header (server-to-server, curl, the Next.js
    // BFF) are unaffected; they still need the Bearer token below.
    const allowedOrigins = config.api.allowedOrigins;
    if (allowedOrigins.length === 0) {
        log.warn('API_ALLOWED_ORIGINS is empty: browser cross-origin requests will be rejected.');
    }
    app.use(cors({
        origin(origin, callback) {
            if (!origin || allowedOrigins.includes('*') || allowedOrigins.includes(origin)) return callback(null, true);
            return callback(null, false);
        },
    }));

    app.use(express.json({ limit: '100kb' }));

    // Request id for log correlation (structured logs pick it up per request).
    app.use((req, res, next) => {
        req.id = crypto.randomUUID().slice(0, 8);
        next();
    });

    app.use(rateLimit({
        windowMs: 60_000,
        limit: config.api.rateLimitPerMinute,
        standardHeaders: true,
        legacyHeaders: false,
    }));

    // ─── Health / Ping (public, minimal: no tag or guild counts) ──────
    const health = (req, res) => {
        res.json({ status: 'ok', uptime: Math.floor(process.uptime()) });
    };
    app.get('/api/health', health);
    app.get('/api/v1/health', health);

    const router = createRouter(client);
    app.use('/api/v1', router);
    app.use('/api', router);

    // RFC 7807 for anything a route did not handle (Express 5 forwards
    // rejected async handlers here automatically).
    app.use((error, req, res, next) => {
        if (res.headersSent) return next(error);
        log.error(`Unhandled API error (${req.method} ${req.originalUrl}) [req ${req.id}]: ${error.message}`);
        return sendError(res, error);
    });

    const server = app.listen(config.api.port, '0.0.0.0', () => {
        log.ok(`REST API server listening on http://0.0.0.0:${config.api.port}`);
    });

    // End every SSE stream when the listener closes (bot.shutdown also calls
    // closeAllConnections(), so keep-alive sockets cannot hold the process).
    server.on('close', () => getSseHub().close());

    return server;
}

module.exports = { startApiServer };
