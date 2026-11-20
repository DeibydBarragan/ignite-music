'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { ApiError, problem } = require('../src/api/contracts');

const targetPath = path.join(
    __dirname,
    '..',
    'rust',
    'crates',
    'nucleo',
    'tests',
    'data',
    'problem-casos.json',
);

const cases = [
    {
        name: 'bad_request',
        doc: problem(400, 'INVALID_SNOWFLAKE', 'must be a Discord snowflake'),
    },
    {
        name: 'unauthorized',
        doc: problem(401, 'UNAUTHORIZED', 'Invalid or missing bearer token'),
    },
    {
        name: 'forbidden',
        doc: problem(403, 'FORBIDDEN', 'Insufficient permissions'),
    },
    {
        name: 'not_found',
        doc: problem(404, 'GUILD_NOT_FOUND', 'The bot is not in this guild'),
    },
    {
        name: 'conflict',
        doc: problem(409, 'ALREADY_PLAYING', 'Queue busy'),
    },
    {
        name: 'payload_too_large',
        doc: problem(413, 'PAYLOAD_TOO_LARGE', 'Payload exceeded limit'),
    },
    {
        name: 'unprocessable',
        doc: problem(422, 'UNPROCESSABLE_ENTITY', 'Cannot parse song structure'),
    },
    {
        name: 'rate_limit',
        doc: problem(429, 'RATE_LIMITED', 'Too many requests, slow down'),
    },
    {
        name: 'bad_gateway',
        doc: problem(502, 'BAD_GATEWAY', 'Upstream gateway error'),
    },
    {
        name: 'service_unavailable',
        doc: problem(503, 'shard_timeout', 'Shard did not answer.'),
    },
    {
        name: 'api_error_custom',
        doc: (() => {
            const err = new ApiError(400, 'CUSTOM_ERR', 'Custom detail', {
                title: 'Custom Title',
                type: 'https://custom.org/err',
            });
            return problem(err.status, err.code, err.message, {
                title: err.title,
                type: err.type,
            });
        })(),
    },
    {
        name: 'extra_fields',
        doc: problem(429, 'RATE_LIMITED', 'Slow down', { retryAfter: 120, limit: 100 }),
    },
    {
        name: 'generic_500_with_message',
        doc: problem(500, 'INTERNAL_ERROR', 'secret internal detail'),
    },
    {
        name: 'generic_500_empty_message',
        doc: problem(500, 'INTERNAL_ERROR', 'Internal server error'),
    },
    {
        name: 'transport_guild_not_found',
        doc: problem(404, 'guild_not_found', 'Guild not found'),
    },
    {
        name: 'transport_not_ready',
        doc: problem(503, 'not_ready', 'System not ready'),
    },
];

fs.mkdirSync(path.dirname(targetPath), { recursive: true });
fs.writeFileSync(targetPath, JSON.stringify(cases, null, 2) + '\n');
console.log(`Generated problem cases reference: ${targetPath}`);
