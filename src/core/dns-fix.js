'use strict';

const dns = require('node:dns');

// On Windows, getaddrinfo with { all: true } (used by undici/fetch in discord.js)
// can fail with EAI_FAIL (-3004) when querying IPv6 (AAAA) on networks without IPv6 routes.
// Enforce family: 4 so Node.js resolves IPv4 addresses reliably and immediately.
if (typeof dns.setDefaultResultOrder === 'function') {
    dns.setDefaultResultOrder('ipv4first');
}

const origLookup = dns.lookup;
dns.lookup = function (host, options, callback) {
    if (typeof options === 'function') {
        callback = options;
        options = { family: 4 };
    } else if (typeof options === 'number') {
        options = { family: options || 4 };
    } else if (typeof options === 'object' && options !== null) {
        options = { ...options, family: 4 };
    } else {
        options = { family: 4 };
    }
    return origLookup.call(this, host, options, callback);
};
