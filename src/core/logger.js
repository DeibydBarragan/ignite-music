'use strict';

const { format } = require('node:util');
const config = require('../config');
const pino = require('pino');

const useColor = Boolean(process.stdout.isTTY) && !config.noColor;
const isProduction = process.env.NODE_ENV === 'production';

let debugEnabled = false;

/**
 * JSON lines in production (log collectors), human-readable output on a
 * developer terminal. Every level flows into pino and `debug` is gated here:
 * setDebug() runs once at startup and must affect loggers created earlier.
 */
const root = pino(
    {
        base: config.shardId !== null ? { shard: config.shardId } : undefined,
        timestamp: pino.stdTimeFunctions.isoTime,
        customLevels: { ok: 35 },
        level: 'debug',
        formatters: { level: (label) => ({ level: label }) },
    },
    isProduction || !useColor
        ? pino.destination({ dest: 1, sync: true })
        : require('pino-pretty')({ colorize: true, translateTime: 'HH:MM:ss', ignore: 'pid,hostname' }),
);

/**
 * pino drops extra arguments when the message has no % placeholder and prints
 * Errors without their stack. Rebuild the message with util.format (exactly
 * what console.log did) and hoist any Error into the `err` field so the full
 * stack always reaches the logs.
 *
 * @param {any[]} args
 * @returns {any[]}
 */
function normalize(args) {
    const error = args.find((arg) => arg instanceof Error);
    if (!error) return [format(...args)];
    const values = args.filter((arg) => arg !== error);
    return [{ err: error }, values.length ? format(...values) : error.message];
}

/**
 * @param {string} scope
 * @returns {any}
 */
function createLogger(scope) {
    const logger = /** @type {any} */ (scope ? root.child({ scope }) : root);
    return {
        debug: (...args) => {
            if (debugEnabled) logger.debug(...normalize(args));
        },
        info: (...args) => logger.info(...normalize(args)),
        ok: (...args) => logger.ok(...normalize(args)),
        warn: (...args) => logger.warn(...normalize(args)),
        error: (...args) => logger.error(...normalize(args)),
        child: (sub) => createLogger(scope ? `${scope}:${sub}` : sub),
    };
}

const logger = createLogger('');
logger.createLogger = createLogger;
logger.setDebug = (value) => {
    debugEnabled = Boolean(value);
};

module.exports = logger;
