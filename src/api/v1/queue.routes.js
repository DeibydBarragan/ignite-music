'use strict';

/**
 * Queue routes. Removal/reorder by trackId is preferred (indexes shift under
 * concurrency); the legacy index/move/jump endpoints are kept verbatim until
 * the dashboard migrates. Every mutation runs under the guild's play lock on
 * the owning shard.
 */

const { sendOpResult } = require('../contracts');
const { snowflakeParams, validateBody, validateParams, schemas } = require('../middleware/validate');

function register(router, { proxy }) {
    router.delete('/guilds/:guildId/queue/items/:trackId', snowflakeParams(), validateParams(schemas.params.guildAndTrack), async (req, res) => {
        const { guildId } = req.params;
        const { trackId } = req.validated.params;
        const result = await proxy.run(guildId, 'queue.removeTrack', { guildId, trackId });
        return sendOpResult(res, result);
    });

    router.delete('/guilds/:guildId/queue/:index', validateParams(schemas.params.guildAndIndex), async (req, res) => {
        const { guildId } = req.params;
        const { index } = req.validated.params;
        const result = await proxy.run(guildId, 'queue.removeIndex', { guildId, index });
        return sendOpResult(res, result);
    });

    router.post('/guilds/:guildId/queue/move', snowflakeParams(), validateBody(schemas.body.move), async (req, res) => {
        const { guildId } = req.params;
        const { from, to } = req.validated.body;
        const result = await proxy.run(guildId, 'queue.move', { guildId, from, to });
        return sendOpResult(res, result);
    });

    router.patch('/guilds/:guildId/queue/order', snowflakeParams(), validateBody(schemas.body.order), async (req, res) => {
        const { guildId } = req.params;
        const { trackId, to } = req.validated.body;
        const result = await proxy.run(guildId, 'queue.order', { guildId, trackId, to });
        return sendOpResult(res, result);
    });

    router.post('/guilds/:guildId/queue/jump', snowflakeParams(), validateBody(schemas.body.jump), async (req, res) => {
        const { guildId } = req.params;
        const { position } = req.validated.body;
        const result = await proxy.run(guildId, 'queue.jump', { guildId, position });
        return sendOpResult(res, result);
    });

    router.delete('/guilds/:guildId/queue', snowflakeParams(), async (req, res) => {
        const { guildId } = req.params;
        const result = await proxy.run(guildId, 'queue.clear', { guildId });
        return sendOpResult(res, result);
    });

    router.post('/guilds/:guildId/queue/undo', snowflakeParams(), async (req, res) => {
        const { guildId } = req.params;
        const result = await proxy.run(guildId, 'queue.undo', { guildId });
        return sendOpResult(res, result);
    });
}

module.exports = { register };
