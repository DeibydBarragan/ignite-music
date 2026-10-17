'use strict';

/**
 * Server-Sent Events hub, keyed by guildId. The dashboard subscribes to
 * GET /api/v1/guilds/:guildId/events and receives `player.sync` snapshots the
 * moment playback state changes, replacing 2s polling.
 *
 * The hub lives in the process that listens on the HTTP port (the API shard);
 * MusicManager publishes locally from every shard process, so in multi-shard
 * deployments only events from the API shard itself stream live — clients
 * still get an authoritative snapshot on (re)connect through the proxy.
 */

const DEFAULT_HEARTBEAT_MS = 25_000;

class SseHub {
    constructor({ heartbeatMs = DEFAULT_HEARTBEAT_MS } = {}) {
        this.heartbeatMs = heartbeatMs;
        this.clients = new Map(); // guildId -> Set<res>
        this.timer = null;
    }

    size(guildId = null) {
        if (guildId !== null) return this.clients.get(guildId)?.size || 0;
        let total = 0;
        for (const set of this.clients.values()) total += set.size;
        return total;
    }

    subscribe(guildId, res) {
        res.set({
            'Content-Type': 'text/event-stream; charset=utf-8',
            'Cache-Control': 'no-cache, no-transform',
            Connection: 'keep-alive',
            'X-Accel-Buffering': 'no',
        });
        res.flushHeaders?.();
        res.write(': connected\n\n');

        let set = this.clients.get(guildId);
        if (!set) {
            set = new Set();
            this.clients.set(guildId, set);
        }
        set.add(res);
        this.#ensureHeartbeat();

        const cleanup = () => {
            const current = this.clients.get(guildId);
            if (!current) return;
            current.delete(res);
            if (current.size === 0) this.clients.delete(guildId);
        };
        res.on('close', cleanup);
        res.on('error', cleanup);
        return cleanup;
    }

    send(res, event, data) {
        res.write(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`);
    }

    publish(guildId, event, data) {
        const set = this.clients.get(guildId);
        if (!set || set.size === 0) return 0;
        let delivered = 0;
        for (const res of set) {
            try {
                this.send(res, event, data);
                delivered++;
            } catch {
                // A broken socket throws on write; the close handler removes it.
            }
        }
        return delivered;
    }

    closeGuild(guildId) {
        const set = this.clients.get(guildId);
        if (!set) return;
        for (const res of set) {
            try { res.end(); } catch { /* already gone */ }
        }
        this.clients.delete(guildId);
    }

    close() {
        if (this.timer) {
            clearInterval(this.timer);
            this.timer = null;
        }
        for (const guildId of [...this.clients.keys()]) this.closeGuild(guildId);
    }

    #ensureHeartbeat() {
        if (this.timer) return;
        this.timer = setInterval(() => {
            for (const set of this.clients.values()) {
                for (const res of set) {
                    try { res.write(': heartbeat\n\n'); } catch { /* removed on close */ }
                }
            }
        }, this.heartbeatMs);
        this.timer.unref();
    }
}

let hub = null;

/** Process-wide singleton so publishers (MusicManager) can reach the hub. */
function getSseHub() {
    hub ??= new SseHub();
    return hub;
}

module.exports = { SseHub, getSseHub, DEFAULT_HEARTBEAT_MS };
