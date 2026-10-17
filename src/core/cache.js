'use strict';

const SWEEP_INTERVAL_MS = 60_000;

// One shared unref'd timer sweeps every live cache, so expired entries are freed
// even when nobody reads them again. WeakRefs keep short-lived caches collectable.
const liveCaches = new Set();
let sweeper = null;
function registerSweeper(cache) {
    liveCaches.add(new WeakRef(cache));
    if (sweeper) return;
    sweeper = setInterval(() => {
        for (const ref of liveCaches) {
            const target = ref.deref();
            if (target) target.sweep();
            else liveCaches.delete(ref);
        }
    }, SWEEP_INTERVAL_MS);
    if (sweeper.unref) sweeper.unref();
}

/**
 * Small TTL cache that also de-duplicates concurrent loads of the same key.
 */
class TTLCache {
    /**
     * @param {{ ttlMs?: number, maxSize?: number }} [options]
     */
    constructor({ ttlMs, maxSize = 1000 } = {}) {
        this.ttlMs = ttlMs;
        this.maxSize = maxSize;
        this.entries = new Map();
        this.pending = new Map();
        registerSweeper(this);
    }

    get(key) {
        const entry = this.entries.get(key);
        if (!entry) return undefined;
        if (entry.expires <= Date.now()) {
            this.entries.delete(key);
            return undefined;
        }
        return entry.value;
    }

    has(key) {
        return this.get(key) !== undefined;
    }

    set(key, value, ttlMs = this.ttlMs) {
        if (value === undefined) return;
        this.entries.delete(key);
        // ±10% jitter so entries created together do not all expire on the same tick.
        const ttl = ttlMs * (0.9 + Math.random() * 0.2);
        this.entries.set(key, { value, expires: Date.now() + ttl });
        while (this.entries.size > this.maxSize) {
            this.entries.delete(this.entries.keys().next().value);
        }
    }

    delete(key) {
        this.entries.delete(key);
    }

    clear() {
        this.entries.clear();
        this.pending.clear();
    }

    sweep() {
        const now = Date.now();
        for (const [key, entry] of this.entries) {
            if (entry.expires <= now) this.entries.delete(key);
        }
    }

    /**
     * Return the cached value or run loader once, sharing the promise between concurrent callers.
     * Rejected loads are never cached: the next call retries the loader.
     */
    async wrap(key, loader, ttlMs = this.ttlMs) {
        const cached = this.get(key);
        if (cached !== undefined) return cached;
        if (this.pending.has(key)) return this.pending.get(key);

        const promise = (async () => {
            try {
                const value = await loader();
                this.set(key, value, ttlMs);
                return value;
            } finally {
                this.pending.delete(key);
            }
        })();
        this.pending.set(key, promise);
        return promise;
    }
}

/**
 * Concurrency limiter with a high-priority lane (user actions) and a low-priority
 * lane (prefetch). Once both lanes have waiters they alternate turns, so a steady
 * stream of user actions cannot starve prefetches. Waiting supports a timeout and
 * an AbortSignal.
 */
class Semaphore {
    constructor(limit) {
        this.limit = Math.max(1, limit);
        this.active = 0;
        this.high = [];
        this.low = [];
        this.lastServed = 'low';
    }

    #takeNext() {
        if (this.lastServed === 'high' && this.low.length) return this.low.shift();
        return this.high.shift() || this.low.shift();
    }

    release() {
        const next = this.#takeNext();
        if (!next) {
            this.active--;
            return;
        }
        this.lastServed = next.priority;
        next.grant();
    }

    /**
     * Reserve a slot. Rejects when the caller's timeout elapses or its signal aborts.
     */
    acquire({ priority = 'high', timeoutMs = 0, signal = null } = {}) {
        if (signal?.aborted) return Promise.reject(signal.reason ?? new Error('Aborted while waiting for a slot'));
        if (this.active < this.limit) {
            this.active++;
            return Promise.resolve();
        }
        return new Promise((resolve, reject) => {
            const low = priority === 'low';
            const lane = low ? this.low : this.high;
            const waiter = { priority: low ? 'low' : 'high' };
            const dispose = () => {
                if (waiter.timer) clearTimeout(waiter.timer);
                if (waiter.onAbort) signal.removeEventListener('abort', waiter.onAbort);
            };
            waiter.grant = () => {
                dispose();
                resolve(undefined);
            };
            waiter.cancel = (error) => {
                dispose();
                const index = lane.indexOf(waiter);
                if (index >= 0) lane.splice(index, 1);
                reject(error);
            };
            if (timeoutMs > 0) {
                waiter.timer = setTimeout(() => {
                    waiter.cancel(new Error(`Timed out after ${Math.round(timeoutMs / 1000)}s waiting for a free slot`));
                }, timeoutMs);
            }
            if (signal) {
                waiter.onAbort = () => waiter.cancel(signal.reason ?? new Error('Aborted while waiting for a slot'));
                signal.addEventListener('abort', waiter.onAbort, { once: true });
            }
            lane.push(waiter);
        });
    }

    /**
     * Run task in a slot. When a slot is free the task starts synchronously, like the
     * previous implementation did; otherwise it starts once acquire() completes.
     */
    run(task, options = {}) {
        const execute = () => {
            let result;
            try {
                result = task();
            } catch (error) {
                this.release();
                throw error;
            }
            return Promise.resolve(result).finally(() => this.release());
        };

        if (!options.signal?.aborted && this.active < this.limit) {
            this.active++;
            try {
                return execute();
            } catch (error) {
                return Promise.reject(error);
            }
        }
        return this.acquire(options).then(execute);
    }
}

module.exports = { TTLCache, Semaphore };
