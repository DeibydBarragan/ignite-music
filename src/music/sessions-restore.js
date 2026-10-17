'use strict';

/**
 * Session persistence (Phase 2.1): periodic save of every live queue and the
 * restore of saved queues for the guilds this shard owns.
 */

const config = require('../config');
const log = require('../core/logger').createLogger('sessions-restore');

function saveAllSessions(manager) {
    if (manager.shuttingDown) return Promise.resolve();
    return Promise.all([...manager.distube.queues.collection.values()]
        .filter((queue) => !queue.stopped && queue.songs.length)
        .map((queue) => manager.sessions.save(queue)));
}

/**
 * Restore saved queues for guilds on this shard.
 */
async function restoreSessions(manager) {
    let restored = 0;
    for (const guildId of manager.sessions.guildIds()) {
        const guild = manager.client.guilds.cache.get(guildId);
        if (!guild) continue; // belongs to another shard
        const data = manager.sessions.load(guildId);
        try {
            if (!data) continue;
            const voiceChannel = await guild.channels.fetch(data.voiceChannelId).catch(() => null);
            const textChannel = data.textChannelId ? await guild.channels.fetch(data.textChannelId).catch(() => null) : null;
            if (!voiceChannel?.isVoiceBased() || !voiceChannel.joinable) continue;
            if (!manager.listenerCount(guild, voiceChannel.id)) continue;

            const songs = manager.sessions.buildSongs(data, guild);
            if (!songs.length) continue;

            const ok = await manager.withPlayLock(guildId, async () => {
                if (manager.getQueue(guildId)) return false; // someone started music meanwhile
                const queue = await manager.distube.queues.create(voiceChannel, textChannel?.isTextBased() ? textChannel : undefined);
                queue.addToQueue(songs);
                queue.volume = data.volume ?? config.bot.defaultVolume;
                queue.repeatMode = [0, 1, 2].includes(data.repeatMode) ? data.repeatMode : 0;
                queue.autoplay = Boolean(data.autoplay);
                for (const name of data.filters || []) {
                    if (manager.distube.filters[name]) queue.filters.collection.set(name, { name, value: manager.distube.filters[name] });
                }
                // Same mechanism Queue#seek uses: the first stream starts at this offset.
                if (songs[0].duration && data.position > 0 && data.position < songs[0].duration - 2) queue._beginTime = data.position;
                await queue.play();
                return true;
            });
            if (ok) restored++;
        } catch (error) {
            log.warn(`Could not restore session for ${guild.name} (${guildId}): ${error.message}`);
        } finally {
            if (!manager.getQueue(guildId)) await manager.sessions.remove(guildId).catch((error) => log.warn(`Could not remove session ${guildId}: ${error.message}`));
        }
    }
    if (restored) log.ok(`Restored ${restored} music session(s)`);
    return restored;
}

module.exports = { saveAllSessions, restoreSessions };
