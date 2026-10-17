'use strict';

const fsp = require('node:fs/promises');
const path = require('node:path');
const { randomUUID } = require('node:crypto');
const log = require('./logger').createLogger('store');

const SAFE_KEY = /^[A-Za-z0-9_-]{1,64}$/;

/**
 * One JSON file per key (guild id). Each guild only lives on one shard, so shards never
 * write the same file and no cross-process locking is needed.
 *
 * Reads are async and happen once through initialize(), which mirrors every file into an
 * in-memory Map: the hot paths (get/keys) never touch the disk. Writes are atomic (unique
 * tmp + fsync + rename) and serialized per key; flush() reports real failures to callers.
 */
class JsonStore {
    constructor(dir) {
        this.dir = dir;
        this.cache = new Map();
        this.writing = new Map();
        this.ready = null;
    }

    file(key) {
        if (!SAFE_KEY.test(String(key))) throw new Error(`Invalid store key: ${key}`);
        return path.join(this.dir, `${key}.json`);
    }

    /**
     * Create the directory and load every existing file into the cache. Idempotent.
     */
    initialize() {
        this.ready ??= (async () => {
            await fsp.mkdir(this.dir, { recursive: true });
            let files = [];
            try {
                files = await fsp.readdir(this.dir);
            } catch (error) {
                log.warn(`Could not list ${this.dir}:`, error.message);
                return this;
            }
            for (const name of files) {
                if (!name.endsWith('.json')) continue;
                const key = name.slice(0, -5);
                if (!SAFE_KEY.test(key) || this.cache.has(key)) continue;
                try {
                    this.cache.set(key, JSON.parse(await fsp.readFile(path.join(this.dir, name), 'utf8')));
                } catch (error) {
                    log.warn(`Could not read ${key}:`, error.message);
                }
            }
            return this;
        })();
        return this.ready;
    }

    get(key) {
        if (!SAFE_KEY.test(String(key))) throw new Error(`Invalid store key: ${key}`);
        return this.cache.get(key) ?? null;
    }

    async set(key, value) {
        await this.initialize();
        this.cache.set(key, value);
        return this.flush(key);
    }

    async delete(key) {
        await this.initialize();
        this.cache.set(key, null);
        return this.flush(key);
    }

    keys() {
        return [...this.cache.entries()]
            .filter(([, value]) => value !== null && value !== undefined)
            .map(([key]) => key);
    }

    /**
     * Serialize writes per key so a slow write never overwrites a newer one.
     * The returned promise rejects when the write really failed.
     */
    flush(key) {
        const previous = this.writing.get(key) || Promise.resolve();
        const next = previous.catch(() => undefined).then(() => this.#write(key));
        this.writing.set(key, next);
        const cleanup = () => {
            if (this.writing.get(key) === next) this.writing.delete(key);
        };
        next.then(cleanup, cleanup);
        return next;
    }

    async #write(key) {
        await this.initialize();
        const file = this.file(key);
        const value = this.cache.get(key);
        if (value === null || value === undefined) {
            await fsp.rm(file, { force: true });
            return;
        }
        const tmp = `${file}.${process.pid}.${randomUUID().slice(0, 8)}.tmp`;
        const handle = await fsp.open(tmp, 'w');
        try {
            await handle.writeFile(JSON.stringify(value), 'utf8');
            await handle.sync();
        } finally {
            await handle.close();
        }
        await fsp.rename(tmp, file);
    }

    async drain() {
        await this.initialize();
        await Promise.all([...this.writing.values()].map((write) => write.catch(() => undefined)));
    }
}

module.exports = { JsonStore };
