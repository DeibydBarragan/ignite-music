'use strict';

/**
 * API v1 router. Mounted under /api/v1 and (temporarily) under /api so the
 * existing dashboard BFF keeps working while it migrates.
 */

const express = require('express');
const { requireAuth, requireUser } = require('../middleware/auth');
const { ShardProxy } = require('../shard-proxy');
const playerRoutes = require('./player.routes');
const queueRoutes = require('./queue.routes');
const discoveryRoutes = require('./discovery.routes');
const voiceRoutes = require('./voice.routes');

function createRouter(client) {
    const router = express.Router();
    const proxy = new ShardProxy(client);

    router.use(requireAuth);
    router.use(requireUser);

    const context = { client, proxy };
    playerRoutes.register(router, context);
    queueRoutes.register(router, context);
    discoveryRoutes.register(router, context);
    voiceRoutes.register(router, context);

    return router;
}

module.exports = { createRouter };
