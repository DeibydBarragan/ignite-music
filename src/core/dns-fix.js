'use strict';

const dns = require('node:dns');
const { promisify } = require('node:util');

// On Windows, getaddrinfo with { all: true } (used by undici/fetch in discord.js)
// can fail with EAI_FAIL (-3004) when querying IPv6 (AAAA) on networks without IPv6 routes.
// Enforce family: 4 so Node.js resolves IPv4 addresses reliably and immediately.
if (typeof dns.setDefaultResultOrder === 'function') {
    dns.setDefaultResultOrder('ipv4first');
}

// Some ISP resolvers (seen: ETB 190.157.x.x) time out intermittently, which
// breaks voice joins, tunnels and API calls with no useful error. Point the
// c-ares resolver at public DNS; the lookup wrapper below falls back to it.
const EXTRA_SERVERS = String(process.env.DNS_SERVERS || '1.1.1.1,8.8.8.8')
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);
try {
    if (EXTRA_SERVERS.length) dns.setServers(EXTRA_SERVERS);
} catch {}

const resolve4 = promisify(dns.resolve4);
const LOOKUP_TIMEOUT_MS = Number(process.env.DNS_LOOKUP_TIMEOUT_MS || 1500) || 1500;

function normalizeLookupArgs(host, options, callback) {
    if (typeof options === 'function') {
        callback = options;
        options = {};
    } else if (typeof options === 'number') {
        options = { family: options || 4 };
    } else if (typeof options !== 'object' || options === null) {
        options = {};
    }
    return { host, options: { ...options, family: 4 }, callback };
}

const origLookup = dns.lookup;
dns.lookup = function (host, options, callback) {
    const args = normalizeLookupArgs(host, options, callback);
    let done = false;
    const finish = (err, address, family) => {
        if (done) return;
        done = true;
        args.callback(err, address, family);
    };
    const timer = setTimeout(() => {
        // System resolver timed out: answer from public DNS instead.
        resolve4(args.host).then(
            (addresses) => {
                if (!addresses || !addresses.length) {
                    finish(Object.assign(new Error(`queryA ETIMEOUT ${args.host}`), { code: 'ETIMEOUT' }));
                    return;
                }
                if (args.options.all) {
                    finish(null, addresses.map((address) => ({ address, family: 4 })));
                } else {
                    finish(null, addresses[0], 4);
                }
            },
            (err) => finish(err)
        );
    }, LOOKUP_TIMEOUT_MS);
    if (timer.unref) timer.unref();
    try {
        origLookup.call(this, args.host, args.options, (err, address, family) => {
            clearTimeout(timer);
            finish(err, address, family);
        });
    } catch (err) {
        clearTimeout(timer);
        finish(err);
    }
};
