'use strict';

/**
 * Discovery routes: search, suggestions, recommendations, top songs, guilds
 * and channel introspection. Search/suggestions run locally on the API shard;
 * guild-scoped operations go through the ShardProxy.
 */

const { sendOpResult, sendProblem } = require('../contracts');
const { snowflakeParams, validateQuery, schemas, inspectTarget } = require('../middleware/validate');

function register(router, { client, proxy }) {
    router.get('/guilds', async (req, res) => {
        const collect = (c) => c.guilds.cache.map((guild) => ({
            id: guild.id,
            name: guild.name,
            icon: guild.iconURL(),
            memberCount: guild.memberCount,
        }));
        // The API listens on shard 0 but must list every shard's guilds.
        const guilds = client.shard?.broadcastEval
            ? (await client.shard.broadcastEval(collect)).flat()
            : collect(client);
        res.json({ guilds });
    });

    router.get('/search/suggest', validateQuery(schemas.query.suggestions), async (req, res) => {
        const q = req.validated.query.q || '';
        try {
            const data = await client.music.apiOps.run('discovery.suggest', { q });
            return res.json(data);
        } catch {
            return res.json({ query: q, suggestions: [] });
        }
    });

    router.get('/search', validateQuery(schemas.query.search), async (req, res) => {
        const q = req.validated.query.q || '';
        const limit = req.validated.query.limit || 6;
        try {
            const data = await client.music.apiOps.run('discovery.search', { q, limit });
            return res.json(data);
        } catch {
            return res.json({ query: q, results: [] });
        }
    });

    router.get('/guilds/:guildId/voice-status', snowflakeParams(), validateQuery(schemas.query.voiceStatus), async (req, res) => {
        const { guildId } = req.params;
        const { userId = null } = req.validated.query;
        const result = await proxy.run(guildId, 'guild.voiceStatus', { guildId, userId });
        return sendOpResult(res, result);
    });

    router.get('/guilds/:guildId/channels', snowflakeParams(), async (req, res) => {
        const { guildId } = req.params;
        const result = await proxy.run(guildId, 'guild.channels', { guildId });
        return sendOpResult(res, result);
    });

    router.get('/guilds/:guildId/recommendations', snowflakeParams(), async (req, res) => {
        const { guildId } = req.params;
        const force = req.query.force === 'true' || req.query.refresh === 'true';
        const result = await proxy.run(guildId, 'discovery.recommendations', { guildId, force });
        return sendOpResult(res, result);
    });

    router.get('/guilds/:guildId/top-songs', snowflakeParams(), validateQuery(schemas.query.topSongs), async (req, res) => {
        const { guildId } = req.params;
        const { timeframe = 'week', limit = 100 } = req.validated.query;
        const result = await proxy.run(guildId, 'discovery.topSongs', { guildId, timeframe, limit });
        return sendOpResult(res, result);
    });

    router.get('/guilds/:guildId/playlist/resolve', snowflakeParams(), validateQuery(schemas.query.resolvePlaylist), async (req, res) => {
        const { guildId } = req.params;
        const { url } = req.validated.query;

        const blocked = inspectTarget(url);
        if (blocked) return sendProblem(res, 400, blocked.code, blocked.detail);

        const result = await proxy.run(guildId, 'playlist.resolve', { guildId, url });
        return sendOpResult(res, result);
    });
}

module.exports = { register };
