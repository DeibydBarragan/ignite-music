'use strict';

/**
 * Queue ingestion (Phase 2.1): resolve a query/URL, join voice in parallel and
 * enqueue under the per-guild play lock with single capacity check and metadata tagging.
 */

const { DisTubeError, Playlist, RepeatMode, isNsfwChannel } = require('distube');
const config = require('../config');
const { assertSupportedUrl } = require('../core/youtube-guards');
const log = require('../core/logger').createLogger('playback');
const presence = require('./presence');

function remaining(manager, guildId) {
    return config.bot.maxQueueSize - (manager.getQueue(guildId)?.songs.length || 0);
}

function queueFull() {
    return new DisTubeError('QUEUE_FULL', `Queue is full (${config.bot.maxQueueSize})`);
}

function requesterUserOf(member) {
    if (!member?.user) return null;
    return {
        id: member.user.id,
        username: member.displayName || member.user.globalName || member.user.username,
        avatar: typeof (member.displayAvatarURL || member.user.displayAvatarURL) === 'function'
            ? (member.displayAvatarURL?.() || member.user?.displayAvatarURL?.())
            : null,
        bot: Boolean(member.user.bot),
    };
}

/**
 * Resolve a query/URL and add it to the queue. Joining the voice channel and resolving the
 * song run in parallel, so the first song starts as early as possible.
 */
async function play(manager, { voiceChannel, textChannel, member, query, position = 0, onQueued = null, skip = false }) {
    const guildId = voiceChannel.guild.id;
    manager.state(guildId).disconnectReason = null;

    assertSupportedUrl(query);

    const requestedAt = Date.now();
    const metadata = { requesterId: member?.id, requesterUser: requesterUserOf(member), guildId };
    const existing = manager.getQueue(guildId);
    const joining = existing ? null : manager.distube.voices.join(voiceChannel).then(() => null, (error) => error);

    let resolved;
    try {
        resolved = await manager.distube.handler.resolve(query, { member, metadata });
    } catch (error) {
        if (joining && !(await joining) && !manager.getQueue(guildId)) presence.scheduleLeave(manager, guildId);
        throw error;
    }

    if (resolved instanceof Playlist) {
        resolved.member = member;
        resolved.metadata = metadata;
        if (Array.isArray(resolved.songs)) {
            for (const s of resolved.songs) {
                s.member = member;
                s.metadata = { ...(s.metadata || {}), ...metadata };
            }
        }
    } else if (resolved) {
        resolved.member = member;
        resolved.metadata = { ...(resolved.metadata || {}), ...metadata };
    }

    const resolvedAt = Date.now();
    if (joining) {
        const joinError = await joining;
        if (joinError) throw joinError;
    }

    // Adding to the queue is serialized per guild: two /play commands at the same moment
    // must not both create a queue (DisTube registers a new queue only after joining).
    return manager.withPlayLock(guildId, async () => {
        const capacity = remaining(manager, guildId);
        if (capacity <= 0) throw queueFull();
        let truncated = false;
        if (resolved instanceof Playlist && resolved.songs.length > capacity) {
            resolved.songs = resolved.songs.slice(0, capacity);
            truncated = true;
        }
        const queue = manager.getQueue(guildId);
        const started = !queue;
        const nsfwAllowed = isNsfwChannel(queue?.textChannel || textChannel);
        // Same rules DisTube applies, checked here so the user is answered before the stream starts.
        if (resolved instanceof Playlist) {
            if (!nsfwAllowed && resolved.songs.every((song) => song.ageRestricted)) throw new DisTubeError('EMPTY_FILTERED_PLAYLIST');
        } else if (!nsfwAllowed && resolved.ageRestricted) {
            throw new DisTubeError('NON_NSFW');
        }

        const first = resolved instanceof Playlist ? resolved.songs[0] : resolved;
        first.timing = { requestedAt, resolvedAt };
        const outcome = {
            resolved,
            truncated,
            started: started || skip,
            position: started || skip ? 0 : (skip ? 1 : position) > 0 ? (skip ? 1 : position) : queue.songs.length,
        };
        // Answer the user now: starting the audio stream can take a moment more.
        if (onQueued) await Promise.resolve(onQueued(outcome)).catch((error) => log.debug(`Reply failed: ${error.message}`));

        await manager.distube.play(voiceChannel, resolved, {
            member,
            textChannel,
            metadata,
            position: skip ? 1 : position,
        });
        const current = manager.getQueue(guildId);
        if (current && !current.textChannel && textChannel) current.textChannel = textChannel;
        if (skip && current && current.songs.length > 1) {
            try {
                await current.skip();
            } catch (e) {
                log.warn(`Skip after play error: ${e.message}`);
            }
        }
        if (skip && current && current.paused) {
            try {
                await current.resume();
            } catch (e) {
                log.warn(`Resume after play error: ${e.message}`);
            }
        }
        if (current) outcome.position = current.songs.indexOf(first);
        return outcome;
    });
}

/**
 * Resolve the next song's stream in the background so it starts without delay.
 */
function prefetchNext(manager, queue) {
    if (queue.repeatMode === RepeatMode.SONG) return;
    const next = queue.songs[1];
    if (!next) return;
    let task = null;
    if (next.stream.playFromSource) {
        if (!next.stream.url && typeof next.plugin?.prefetch === 'function') task = next.plugin.prefetch(next);
    } else if (!next.stream.song && typeof next.plugin?.createSearchQuery === 'function') {
        task = manager.youtube.prefetchQuery(next.plugin.createSearchQuery(next), queue.id);
    }
    task?.catch((error) => log.debug(`Prefetch failed for ${next.name}: ${error.message}`));
}

module.exports = { play, prefetchNext, remaining, queueFull };
