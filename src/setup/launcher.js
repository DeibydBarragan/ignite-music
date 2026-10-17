'use strict';

require('../core/dns-fix');

const fs = require('node:fs');
const path = require('node:path');
const { ShardingManager, RESTJSONErrorCodes } = require('discord.js');
const config = require('../config');
const logger = require('../core/logger');
const { deployCommands } = require('../deploy');
const preflight = require('./preflight');
const { SetupError } = require('./setup-error');
const { StreamRelay } = require('../music/relay');
const tunnel = require('../api/tunnel');
const { beat } = require('../api/announce');

const log = logger.createLogger('manager');
logger.setDebug(config.debug);

const SHUTDOWN_GRACE_MS = 10_000;

/**
 * v16 and older downloaded every song into audio_cache/. Nothing uses it any more, so free the disk space.
 */
function removeLegacyCache() {
    const dir = path.join(config.root, 'audio_cache');
    if (!fs.existsSync(dir)) return;
    try {
        fs.rmSync(dir, { recursive: true, force: true });
        log.info('Removed the old audio_cache folder (no longer used).');
    } catch (error) {
        log.warn('Could not remove the old audio_cache folder:', error.message);
    }
}

/**
 * Shard-only environment values. discord.js copies the manager's process.env
 * into every shard and offers no per-spawn environment option, so everything
 * shards must see is published here, in one place, right before spawning.
 * announce.js resolves the same tunnel-first URL at runtime.
 */
function buildShardEnv(config) {
    process.env.SHARDING_MANAGER = '1';
    process.env.SHARDING_MANAGER_MODE = config.sharding.mode;
    const publicUrl = tunnel.getTunnelUrl() || config.api.publicUrl;
    if (publicUrl) process.env.PUBLIC_API_URL = publicUrl;
    return process.env;
}

/**
 * Entry point. The bot always runs under the ShardingManager: small bots get one shard,
 * large bots get as many as Discord recommends, and crashed shards restart automatically.
 */
async function main() {
    removeLegacyCache();
    StreamRelay.cleanAll();
    // Checks the configuration, downloads yt-dlp/ffmpeg when needed and stops with
    // instructions when something must be fixed by hand.
    await preflight.run();

    try {
        await deployCommands();
    } catch (error) {
        if (error.code === RESTJSONErrorCodes.MissingAccess || error.code === RESTJSONErrorCodes.UnknownGuild) {
            log.error(`Slash commands could not be registered in GUILD_ID ${config.discord.guildId}: the bot is not in that server. Invite it there or leave GUILD_ID empty.`);
        } else {
            log.error('Slash command registration failed:', error.message);
        }
    }

    if (config.api.enabled && config.api.autoTunnel) {
        // announce.js prefers the live tunnel URL over the static PUBLIC_API_URL,
        // so no config mutation is needed here.
        const tunnelUrl = await tunnel.startTunnel(config.api.port);
        if (tunnelUrl) await beat();
    }

    const manager = new ShardingManager(path.join(__dirname, '..', 'bot.js'), {
        token: config.discord.token,
        totalShards: config.sharding.totalShards,
        shardList: config.sharding.shardList,
        mode: config.sharding.mode,
        respawn: config.sharding.respawn,
        execArgv: process.execArgv,
    });

    manager.on('shardCreate', (shard) => {
        log.info(`Launching shard ${shard.id}`);
        shard.on('death', (/** @type {any} */ child) => log.warn(`Shard ${shard.id} exited (code ${child.exitCode ?? 'unknown'})${manager.respawn ? ', restarting' : ''}`));
        shard.on('error', (error) => log.error(`Shard ${shard.id} error:`, error));
    });

    let stopping = false;
    const stop = async () => {
        if (stopping) return;
        stopping = true;
        manager.respawn = false;
        log.info('Stopping all shards...');
        if (config.api.autoTunnel) {
            tunnel.stopTunnel();
        }

        // Each shard saves its sessions, closes its API server (dropping
        // keep-alive sockets) and destroys its client before replying.
        await Promise.race([
            manager.broadcastEval((client) => /** @type {any} */ (client).shutdown?.()).catch(() => undefined),
            new Promise((resolve) => setTimeout(resolve, SHUTDOWN_GRACE_MS)),
        ]);

        // Managed shards intentionally ignore SIGTERM, so the fallback after
        // the grace window is SIGKILL: nothing outlives the manager.
        for (const shard of manager.shards.values()) {
            const child = shard.process;
            if (child && child.exitCode === null && child.signalCode === null) {
                log.warn(`Shard ${shard.id} did not exit in time; sending SIGKILL.`);
                try {
                    child.kill('SIGKILL');
                } catch {
                    // already gone
                }
            } else if (!child && shard.worker) {
                log.warn(`Shard ${shard.id} did not exit in time; terminating worker.`);
                shard.worker.terminate().catch(() => undefined);
            }
        }
        process.exit(0);
    };
    process.once('SIGINT', stop);
    process.once('SIGTERM', stop);

    process.on('unhandledRejection', (reason) => log.error('Manager unhandled rejection:', reason));
    process.on('uncaughtException', (error) => log.error('Manager uncaught exception:', error));

    buildShardEnv(config);
    await manager.spawn({
        amount: config.sharding.totalShards,
        delay: config.sharding.spawnDelay,
        timeout: config.sharding.spawnTimeout,
    });
    log.ok(`${manager.shards.size} shard(s) running`);
}

module.exports = {
    buildShardEnv,
    main: () => main().catch((error) => {
        // Preflight failures were already explained on stderr by report.js.
        if (error instanceof SetupError) process.exit(1);
        log.error('Failed to start:', error);
        process.exit(1);
    }),
};
