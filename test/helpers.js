'use strict';

const path = require('node:path');
const os = require('node:os');
const fs = require('node:fs');

// Isolated data dir for every test process.
process.env.DATA_DIR = fs.mkdtempSync(path.join(os.tmpdir(), 'musicbot-test-'));

// ─── Global @supabase/supabase-js mock ─────────────────────────────────────
// .env may point at the production project, and src/core/supabase.js creates
// its client eagerly. Intercept the package so `npm test` can never read or
// write the real database: every client is an inert, chainable stub that
// resolves empty results. Individual tests can inspect `supabaseMock.calls`.
const Module = require('node:module');

const supabaseMock = { calls: [] };

supabaseMock.createClient = () => ({
    from(table) {
        const call = { table, operations: [] };
        supabaseMock.calls.push(call);
        const query = {};
        for (const method of ['select', 'insert', 'update', 'upsert', 'delete', 'eq', 'neq', 'in', 'is', 'order', 'limit', 'gte', 'lte', 'range']) {
            query[method] = (...args) => {
                call.operations.push([method, ...args]);
                return query;
            };
        }
        query.single = () => Promise.resolve({ data: null, error: null });
        query.maybeSingle = () => Promise.resolve({ data: null, error: null });
        query.then = (resolve, reject) => Promise.resolve({ data: [], error: null }).then(resolve, reject);
        return query;
    },
    rpc(name, args) {
        supabaseMock.calls.push({ table: `rpc:${name}`, operations: [args] });
        return { then: (resolve, reject) => Promise.resolve({ data: [], error: null }).then(resolve, reject) };
    },
    auth: { persistSession: false },
});

const originalLoad = Module._load;
Module._load = function (request, parent, isMain) {
    if (request === '@supabase/supabase-js') return supabaseMock;
    return originalLoad.call(this, request, parent, isMain);
};

const { Song } = require('distube');

const ids = { guild: '111111111111111111', user: '222222222222222222', bot: '333333333333333333', voice: '444444444444444444', text: '555555555555555555' };

function fakeMember(id = ids.user, bot = false) {
    return { id, user: { id, bot }, guild: { id: ids.guild } };
}

function makeSong(n, extra = {}) {
    const plugin = extra.plugin || { type: 'extractor' };
    return new Song({
        plugin,
        source: 'youtube',
        playFromSource: true,
        id: `vid${String(n).padStart(8, '0')}`,
        name: `Song ${n}`,
        url: `https://www.youtube.com/watch?v=vid${String(n).padStart(8, '0')}`,
        duration: 180 + n,
        thumbnail: `https://i.ytimg.com/vi/vid${n}/hqdefault.jpg`,
        uploader: { name: `Artist ${n}` },
        ...extra,
    }, { metadata: { requesterId: ids.user } });
}

const t = (key, vars = {}) => `${key}${Object.keys(vars).length ? JSON.stringify(vars) : ''}`;
t.lang = 'en';

module.exports = { ids, fakeMember, makeSong, t, supabaseMock };
