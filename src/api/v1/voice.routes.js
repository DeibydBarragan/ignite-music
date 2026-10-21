'use strict';

/**
 * Voice settings routes: get, update and reset customizable guild voice configuration.
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
}

module.exports = { register };
