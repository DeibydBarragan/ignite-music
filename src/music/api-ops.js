'use strict';

/**
 * Registry of guild-scoped operations the REST API can execute. Operations
 * run in the shard that owns the guild (see src/api/shard-proxy.js), so each
 * one receives the MusicManager and must return JSON-serializable data.
 * Rejections travel back with their `code` so routes can map them to HTTP
 * statuses (RFC 7807) without leaking internals.
 */
class ApiOps {
    constructor(music) {
        this.music = music;
        this.ops = new Map();
    }

    register(name, fn) {
        this.ops.set(name, fn);
        return this;
    }

    has(name) {
        return this.ops.has(name);
    }

    async run(name, payload) {
        const op = this.ops.get(name);
        if (!op) {
            const error = new Error(`Unknown API operation: ${name}`);
            error.code = 'unknown_op';
            throw error;
        }
        return op(this.music, payload ?? {});
    }
}

module.exports = { ApiOps };
