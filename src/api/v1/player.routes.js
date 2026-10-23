'use strict';

/**
 * Player routes: state, play and granular control. All of them execute on the
 * shard that owns the guild through the ShardProxy and answer with the exact
 * JSON shapes the dashboard already consumes.
 */

const { sendOpResult, sendProblem } = require('../contracts');
const { getSseHub } = require('../sse');
const { snowflakeParams, validateBody, schemas, inspectTarget } = require('../middleware/validate');
const { isDevMode } = require('../../core/dev-mode');

function register(router, { proxy }) {
    router.get('/guilds/:guildId/player', snowflakeParams(), async (req, res) => {
        const { guildId } = req.params;
        const result = await proxy.run(guildId, 'player.state', { guildId });
        return sendOpResult(res, result);
    });

    router.post('/guilds/:guildId/play', snowflakeParams(), validateBody(schemas.body.play), async (req, res) => {
        const { guildId } = req.params;
        const body = req.validated.body;

        const blocked = inspectTarget(body.query);
        if (blocked) return sendProblem(res, 400, blocked.code, blocked.detail);

        const result = await proxy.run(guildId, 'player.play', {
            guildId,
            query: body.query,
            voiceChannelId: body.voiceChannelId || null,
            // Validated X-User-Discord-Id wins; body.userId stays for legacy clients.
            userId: req.discordUserId || body.userId || null,
            next: Boolean(body.next),
            skip: Boolean(body.skip),
            devMode: req.headers['x-dev-mode'] === 'true' || Boolean(body.devMode) || Boolean(req.discordUserId && isDevMode(req.discordUserId)),
        });
        return sendOpResult(res, result);
    });

    router.post('/guilds/:guildId/control', snowflakeParams(), validateBody(schemas.body.control), async (req, res) => {
        const { guildId } = req.params;
        const { action, value } = req.validated.body;
        const result = await proxy.run(guildId, 'player.control', { guildId, action, value });
        return sendOpResult(res, result);
    });

    // Server-Sent Events: initial authoritative snapshot, then player.sync
    // events published by MusicManager on every playback/queue change.
    router.get('/guilds/:guildId/events', snowflakeParams(), async (req, res) => {
        const { guildId } = req.params;
        const result = await proxy.run(guildId, 'player.state', { guildId });
        if (!result.ok) return sendOpResult(res, result);

        const hub = getSseHub();
        hub.subscribe(guildId, res);
        hub.send(res, 'player.sync', result.data);
    });
}

module.exports = { register };
