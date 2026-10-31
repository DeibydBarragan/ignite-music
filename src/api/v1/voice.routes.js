'use strict';

/**
 * Voice settings and dev logs routes: get, update and reset customizable guild voice configuration,
 * and view live voice interaction logs for Developer Mode.
 * Forwards requests through the ShardProxy to the owning shard.
 */

const { sendOpResult } = require('../contracts');
const { snowflakeParams, validateBody, schemas } = require('../middleware/validate');

function register(router, { proxy }) {
    router.get('/guilds/:guildId/voice', snowflakeParams(), async (req, res) => {
        const { guildId } = req.params;
        const result = await proxy.run(guildId, 'voice.get', { guildId });
        return sendOpResult(res, result);
    });

    router.put('/guilds/:guildId/voice', snowflakeParams(), validateBody(schemas.body.voiceUpdate), async (req, res) => {
        const { guildId } = req.params;
        const body = req.validated.body;
        const result = await proxy.run(guildId, 'voice.update', { guildId, ...body });
        return sendOpResult(res, result);
    });

    router.post('/guilds/:guildId/voice/reset', snowflakeParams(), async (req, res) => {
        const { guildId } = req.params;
        const result = await proxy.run(guildId, 'voice.reset', { guildId });
        return sendOpResult(res, result);
    });

    router.get('/guilds/:guildId/voice/logs', snowflakeParams(), async (req, res) => {
        const { guildId } = req.params;
        const limit = parseInt(req.query.limit, 10) || 50;
        const result = await proxy.run(guildId, 'voice.logs', { guildId, limit });
        return sendOpResult(res, result);
    });

    router.delete('/guilds/:guildId/voice/logs', snowflakeParams(), async (req, res) => {
        const { guildId } = req.params;
        const result = await proxy.run(guildId, 'voice.clearLogs', { guildId });
        return sendOpResult(res, result);
    });

    router.post('/guilds/:guildId/voice/join', snowflakeParams(), async (req, res) => {
        const { guildId } = req.params;
        const channelId = req.body?.channelId || null;
        const userId = req.discordUserId || req.body?.userId || null;
        const result = await proxy.run(guildId, 'voice.join', { guildId, channelId, userId });
        return sendOpResult(res, result);
    });

    router.post('/guilds/:guildId/voice/leave', snowflakeParams(), async (req, res) => {
        const { guildId } = req.params;
        const result = await proxy.run(guildId, 'voice.leave', { guildId });
        return sendOpResult(res, result);
    });
}

module.exports = { register };
