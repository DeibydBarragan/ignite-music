'use strict';

require('./core/dns-fix');

const { ActivityType, Client, Events, GatewayIntentBits } = require('discord.js');
const config = require('./config');
const logger = require('./core/logger');
const { loadCommands } = require('./commands');
const { MusicManager } = require('./music/manager');
const { onInteraction } = require('./handlers/interactions');
const { registerApiOps } = require('./api/v1/ops');
const { startApiServer } = require('./api/server');
const { startAnnouncer } = require('./api/announce');
const ytdlp = require('./music/ytdlp');

const log = logger.createLogger('bot');
logger.setDebug(config.debug);

// Start minting the PO token while the client logs in (shards run without preflight).
require('./music/pot').startPoTokenRenewal();

/**
 * Create the Discord client with the music system attached. Exported for tests.
 */
function createClient() {
    const client = new Client({
        // Only the intents the bot needs: no privileged intents required.
        intents: [GatewayIntentBits.Guilds, GatewayIntentBits.GuildVoiceStates],
        presence: { activities: [{ name: config.bot.status, type: ActivityType.Listening }] },
    });

    client.commands = loadCommands();
    client.commandIds = new Map();
    client.music = new MusicManager(client);
    // Every shard can serve REST operations for the guilds it owns.
    registerApiOps(client.music);

    client.on(Events.InteractionCreate, onInteraction);
    client.on(Events.VoiceStateUpdate, (oldState, newState) => {
        client.music.handleVoiceStateUpdate(oldState, newState).catch((error) => log.warn('Voice state handling failed:', error.message));
    });
    client.on(Events.Error, (error) => log.error('Client error:', error));
    client.on(Events.Warn, (message) => log.warn(message));

    client.once(Events.ClientReady, async (ready) => {
        log.ok(`Logged in as ${ready.user.tag} · ${ready.guilds.cache.size} servers on this shard`);
        try {
            const commands = await ready.application.commands.fetch(config.discord.guildId ? { guildId: config.discord.guildId } : {});
            for (const command of commands.values()) client.commandIds.set(command.name, command.id);
        } catch (error) {
            log.warn('Could not fetch command ids:', error.message);
        }
        await client.music.restoreSessions().catch((error) => log.error('Session restore failed:', error));

        if (!client.shard || client.shard.ids[0] === 0) {
            client.apiServer = startApiServer(client);
            client.announcer = startAnnouncer();
        }
    });

    let stopping = null;
    client.shutdown = () => {
        stopping ??= (async () => {
            log.info('Shutting down...');
            if (client.announcer) {
                clearInterval(client.announcer);
            }
            if (client.apiServer) {
                client.apiServer.close();
                // Drop keep-alive sockets so the listener actually closes (Node 18.2+).
                client.apiServer.closeAllConnections?.();
            }
            await client.music.shutdown().catch((error) => log.error('Music shutdown failed:', error));
            await client.destroy();
            return true;
        })();
        return stopping;
    };

    return client;
}

async function start() {
    if (!config.discord.token) {
        log.error('DISCORD_TOKEN is missing. Copy .env.example to .env and fill it in.');
        process.exit(1);
    }

    const ytdlpStatus = await ytdlp.version();
    if (ytdlpStatus.version) log.info(`yt-dlp ${ytdlpStatus.version} (${ytdlp.binary})`);
    else log.error(`yt-dlp at "${ytdlp.binary}" does not work: ${ytdlpStatus.error}. YouTube will not play until this is fixed.`);

    const client = createClient();
    const managed = config.sharding.managed;

    const exit = async (code) => {
        await client.shutdown();
        process.exit(code);
    };

    if (managed) {
        // The sharding manager coordinates shutdown; ignore terminal signals here so a
        // shard is never respawned halfway through stopping. Exit if the manager dies.
        process.on('SIGINT', () => undefined);
        process.on('SIGTERM', () => undefined);
        if (process.send) process.on('disconnect', () => exit(0));
    } else {
        process.once('SIGINT', () => exit(0));
        process.once('SIGTERM', () => exit(0));
    }

    process.on('unhandledRejection', (reason) => log.error('Unhandled rejection:', reason));
    process.on('uncaughtException', (error) => {
        // State is undefined after an uncaught exception: save sessions and restart cleanly.
        log.error('Uncaught exception:', error);
        exit(1);
    });

    await client.login(config.discord.token);
    return client;
}

if (require.main === module) start();

module.exports = { createClient, start };
