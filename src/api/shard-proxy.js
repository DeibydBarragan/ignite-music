'use strict';

/**
 * Guild-scoped API operations must run on the shard that owns the guild, but
 * the REST API only listens on shard 0. This proxy locates the owning shard
 * with broadcastEval (hard timeout) and forwards operations to MusicManager's
 * apiOps registry, which exists in every shard. Guilds that already live on
 * shard 0 take a local fast path with no IPC.
 *
 * Everything crossing the shard boundary must be JSON-serializable: the
 * context is embedded in the evaluated script and results travel over IPC.
 */

const DEFAULT_TIMEOUT_MS = 5000;
const DEFAULT_CACHE_TTL_MS = 30_000;

// Evaluated inside each shard: the owning shard reports its own id, others null.
const LOCATE = (client, guildId) => (client.guilds.cache.has(guildId) ? client.shard?.ids?.[0] ?? 0 : null);

// Evaluated inside the owning shard: dispatch through the manager's api ops.
// Returns null on shards that do not have the guild so the proxy can tell
// "wrong shard" apart from real operation errors.
const DISPATCH = (client, context) => {
    if (!client.guilds.cache.has(context.guildId)) return null;
    const ops = client.music?.apiOps;
    if (!ops) return { ok: false, code: 'not_ready', message: 'Music system is not ready on this shard' };
    return Promise.resolve()
        .then(() => ops.run(context.op, context.payload))
        .then(
            (data) => ({ ok: true, data }),
            (error) => ({
                ok: false,
                code: error?.code || 'op_failed',
                status: error?.status || undefined,
                message: error?.message || String(error),
            }),
        );
};

class ShardProxy {
    constructor(client, { timeoutMs = DEFAULT_TIMEOUT_MS, cacheTtlMs = DEFAULT_CACHE_TTL_MS } = {}) {
        this.client = client;
        this.timeoutMs = timeoutMs;
        this.cacheTtlMs = cacheTtlMs;
        this.cache = new Map();
    }

    isLocal(guildId) {
        return this.client?.guilds?.cache?.has(guildId) === true;
    }

    /**
     * Id of the shard that owns the guild, or null when no shard has it.
     * Positive results are cached; the TTL bounds staleness after guild moves.
     */
    async locate(guildId) {
        const cached = this.cache.get(guildId);
        if (cached && cached.expiresAt > Date.now()) return cached.shardId;

        if (this.isLocal(guildId)) {
            this.cache.set(guildId, { shardId: 0, expiresAt: Date.now() + this.cacheTtlMs });
            return 0;
        }

        const shard = this.client?.shard;
        if (!shard?.broadcastEval) return null;

        const results = await this.#race(
            shard.broadcastEval(LOCATE, { context: guildId }),
            `Shard lookup timed out after ${this.timeoutMs}ms`,
        );
        const owner = (Array.isArray(results) ? results : [results]).find((id) => typeof id === 'number');
        if (typeof owner !== 'number') return null;

        this.cache.set(guildId, { shardId: owner, expiresAt: Date.now() + this.cacheTtlMs });
        return owner;
    }

    /**
     * Run an apiOp on the guild's shard.
     *
     * @returns {Promise<{ok: true, data: any} | {ok: false, code: string, message: string}>}
     */
    async run(guildId, op, payload = null) {
        if (this.isLocal(guildId)) {
            return DISPATCH(this.client, { guildId, op, payload });
        }

        try {
            const shardId = await this.locate(guildId);
            if (shardId === null) {
                return { ok: false, code: 'guild_not_found', message: 'The bot is not in this guild' };
            }

            const response = await this.#evaluate(shardId, { guildId, op, payload });
            if (response !== null) return response;

            // Stale cache: the guild moved shards or left. Refresh once.
            this.cache.delete(guildId);
            const again = await this.locate(guildId);
            if (again === null || again === shardId) {
                return { ok: false, code: 'guild_not_found', message: 'The bot is not in this guild' };
            }
            return (await this.#evaluate(again, { guildId, op, payload }))
                ?? { ok: false, code: 'guild_not_found', message: 'The bot is not in this guild' };
        } catch (error) {
            return { ok: false, code: 'shard_timeout', message: error.message };
        }
    }

    async #evaluate(shardId, context) {
        const results = await this.#race(
            this.client.shard.broadcastEval(DISPATCH, { shard: shardId, context }),
            `Shard ${shardId} did not answer within ${this.timeoutMs}ms`,
        );
        const list = Array.isArray(results) ? results : [results];
        return list.find((entry) => entry !== null && entry !== undefined) ?? null;
    }

    /**
     * Reject after timeoutMs unless the operation settles first. The timer is
     * never unref'd (the reject must fire with an idle event loop) and is
     * always cleared so a completed call never delays shutdown.
     */
    async #race(operation, message) {
        let timer;
        try {
            return await Promise.race([
                operation,
                new Promise((_, reject) => {
                    timer = setTimeout(() => reject(new Error(message)), this.timeoutMs);
                }),
            ]);
        } finally {
            clearTimeout(timer);
        }
    }
}

module.exports = { ShardProxy, DEFAULT_TIMEOUT_MS, DEFAULT_CACHE_TTL_MS };
