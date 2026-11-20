'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.join(__dirname, '..');
const read = (file) => fs.readFileSync(path.join(root, file), 'utf8');

function operationsInOpenApi(source) {
    const operations = new Set();
    let currentPath = null;
    for (const line of source.split(/\r?\n/)) {
        const pathMatch = line.match(/^  (\/api\/v1\/[^:]+):\s*$/);
        if (pathMatch) {
            currentPath = pathMatch[1];
            continue;
        }
        const methodMatch = line.match(/^    (get|post|put|patch|delete):\s*$/);
        if (methodMatch && currentPath) operations.add(`${methodMatch[1].toUpperCase()} ${currentPath}`);
        if (/^components:/.test(line)) currentPath = null;
    }
    return operations;
}

test('OpenAPI covers every method and path registered by Rust and the TypeScript API', () => {
    const documented = operationsInOpenApi(read('openapi.yaml'));
    const expected = [
        'GET /api/v1/health',
        'GET /api/v1/guilds/{guildId}/player',
        'POST /api/v1/guilds/{guildId}/play',
        'POST /api/v1/guilds/{guildId}/control',
        'GET /api/v1/guilds/{guildId}/events',
        'DELETE /api/v1/guilds/{guildId}/queue',
        'POST /api/v1/guilds/{guildId}/queue/undo',
        'DELETE /api/v1/guilds/{guildId}/queue/{index}',
        'DELETE /api/v1/guilds/{guildId}/queue/items/{trackId}',
        'POST /api/v1/guilds/{guildId}/queue/move',
        'PATCH /api/v1/guilds/{guildId}/queue/order',
        'POST /api/v1/guilds/{guildId}/queue/jump',
        'GET /api/v1/guilds',
        'GET /api/v1/search',
        'GET /api/v1/search/suggest',
        'GET /api/v1/guilds/{guildId}/top-songs',
        'GET /api/v1/guilds/{guildId}/voice-status',
        'GET /api/v1/guilds/{guildId}/channels',
        'GET /api/v1/guilds/{guildId}/recommendations',
        'GET /api/v1/guilds/{guildId}/playlist/resolve',
        'GET /api/v1/search/playlists',
        'GET /api/v1/guilds/{guildId}/voice',
        'PUT /api/v1/guilds/{guildId}/voice',
        'POST /api/v1/guilds/{guildId}/voice/reset',
        'GET /api/v1/guilds/{guildId}/voice/logs',
        'DELETE /api/v1/guilds/{guildId}/voice/logs',
        'POST /api/v1/guilds/{guildId}/voice/join',
        'POST /api/v1/guilds/{guildId}/voice/leave',
        'GET /api/v1/user/dev-mode',
        'POST /api/v1/user/dev-mode',
    ];
    assert.equal(expected.length, 30);
    assert.deepEqual([...documented].sort(), [...expected].sort());

    const typescriptRouter = read('test/api.test.js');
    const rustRouter = read('rust/crates/bot/src/api/rutas/mod.rs');
    assert.match(typescriptRouter, /app\.use\('\/api\/v1',\s*router\)/);
    assert.match(typescriptRouter, /app\.use\('\/api',\s*router\)/);
    assert.match(rustRouter, /nest\("\/api\/v1"/);
    assert.match(rustRouter, /nest\("\/api"/);
});

test('OpenAPI snowflake patterns are single-escaped and accept real Discord IDs', () => {
    const patterns = [...read('openapi.yaml').matchAll(/pattern:\s*'([^']*)'/g)].map((match) => match[1]);
    for (const pattern of patterns) {
        assert.ok(!pattern.includes('\\\\'), `patron con doble barra invertida: ${pattern}`);
    }
    const snowflakes = patterns.filter((pattern) => pattern.includes('{17,20}'));
    assert.ok(snowflakes.length > 0);
    for (const pattern of snowflakes) {
        assert.match('123456789012345678', new RegExp(pattern));
    }
});
