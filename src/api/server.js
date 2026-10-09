'use strict';

const express = require('express');
const cors = require('cors');
const { RepeatMode } = require('distube');
const config = require('../config');
const actions = require('../music/actions');
const i18n = require('../core/i18n');
const logger = require('../core/logger');

const log = logger.createLogger('api');

/**
 * Format a DisTube Song into the Ignite Frontend data contract.
 */
function formatSong(song) {
    if (!song) return null;
    return {
        id: song.id || song.url,
        title: song.name || 'Unknown Title',
        artist: song.uploader?.name || 'Unknown Artist',
        albumArt: song.thumbnail || 'https://images.unsplash.com/photo-1470225620780-dba8ba36b745?w=300&q=80',
        coverUrl: song.thumbnail || null,
        duration: song.duration || 0,
        formattedDuration: song.formattedDuration || '00:00',
        source: song.source || 'youtube',
        url: song.url,
        category: 'hits',
        isLive: Boolean(song.isLive),
        requestedBy: song.user ? {
            id: song.user.id,
            username: song.user.username,
            avatar: typeof song.user.displayAvatarURL === 'function' ? song.user.displayAvatarURL() : null,
        } : null,
    };
}

/**
 * Start the internal REST API server attached to the Discord client.
 */
function startApiServer(client) {
    if (!config.api.enabled) {
        log.info('API server is disabled in config.');
        return null;
    }

    const app = express();
    app.use(cors({ origin: '*' }));
    app.use(express.json());

    // ─── Authentication Middleware ────────────────────────────────────
    const requireAuth = (req, res, next) => {
        const header = req.headers.authorization || req.headers['x-api-key'];
        const token = header?.startsWith('Bearer ') ? header.slice(7) : header;

        if (!config.api.secret || token === config.api.secret) {
            return next();
        }
        return res.status(401).json({ error: 'Unauthorized: Invalid or missing API secret' });
    };

    // ─── Health / Ping ────────────────────────────────────────────────
    app.get('/api/health', (req, res) => {
        res.json({
            status: 'ok',
            uptime: Math.floor(process.uptime()),
            bot: client.user ? { id: client.user.id, tag: client.user.tag } : null,
            guildsCount: client.guilds.cache.size,
            timestamp: new Date().toISOString(),
        });
    });

    // Apply auth to all subsequent endpoints
    app.use('/api', requireAuth);

    // ─── Guilds List ──────────────────────────────────────────────────
    app.get('/api/guilds', (req, res) => {
        const guilds = client.guilds.cache.map((guild) => ({
            id: guild.id,
            name: guild.name,
            icon: guild.iconURL(),
            memberCount: guild.memberCount,
        }));
        res.json({ guilds });
    });

    // ─── Voice Channel Status ─────────────────────────────────────────
    app.get('/api/guilds/:guildId/voice-status', async (req, res) => {
        const { guildId } = req.params;
        const { userId } = req.query;

        const guild = client.guilds.cache.get(guildId);
        if (!guild) return res.status(404).json({ error: 'Guild not found' });

        const botMember = guild.members.me;
        const botChannel = botMember?.voice?.channel;

        let userChannel = null;
        if (userId) {
            try {
                const member = guild.members.cache.get(userId) || await guild.members.fetch(userId).catch(() => null);
                userChannel = member?.voice?.channel;
            } catch {
                // ignore
            }
        }

        res.json({
            bot: botChannel ? { id: botChannel.id, name: botChannel.name } : null,
            user: userChannel ? { id: userChannel.id, name: userChannel.name } : null,
            canControl: Boolean(botChannel && userChannel && botChannel.id === userChannel.id),
        });
    });

    // ─── Voice Channels of a Guild (canales + gente conectada) ───
    app.get('/api/guilds/:guildId/channels', async (req, res) => {
        const { guildId } = req.params;

        const guild = client.guilds.cache.get(guildId);
        if (!guild) return res.status(404).json({ error: 'Guild not found' });

        try {
            await guild.channels.fetch().catch(() => null);
        } catch {
            // Seguir con la caché si el fetch falla
        }

        const botId = client.user?.id;
        const channels = [...guild.channels.cache.filter((c) => c.isVoiceBased()).values()]
            .sort((a, b) => (a.position ?? 0) - (b.position ?? 0))
            .map((c) => {
                const humans = c.members.filter((m) => !m.user.bot);
                return {
                    id: c.id,
                    name: c.name,
                    bitrate: c.bitrate,
                    userLimit: c.userLimit || 0,
                    userCount: humans.size,
                    botPresent: botId ? c.members.has(botId) : false,
                    members: [...humans.values()].slice(0, 25).map((m) => ({
                        id: m.id,
                        username: m.user.username,
                        avatar:
                            typeof m.user.displayAvatarURL === 'function'
                                ? m.user.displayAvatarURL()
                                : null,
                        bot: m.user.bot,
                    })),
                };
            });

        res.json({ guildId, total: channels.length, channels });
    });

    // ─── Player State & Queue ─────────────────────────────────────────
    app.get('/api/guilds/:guildId/player', (req, res) => {
        const { guildId } = req.params;
        const queue = client.music?.distube?.getQueue(guildId);

        if (!queue) {
            return res.json({
                hasQueue: false,
                isPlaying: false,
                isPaused: false,
                currentTime: 0,
                duration: 0,
                volume: config.bot.defaultVolume,
                repeatMode: 'off',
                autoplay: false,
                filter: null,
                currentSong: null,
                queue: [],
            });
        }

        const currentSong = queue.songs[0] ? formatSong(queue.songs[0]) : null;
        const upcoming = queue.songs.slice(1).map(formatSong);

        let repeatMode = 'off';
        if (queue.repeatMode === RepeatMode.SONG) repeatMode = 'track';
        else if (queue.repeatMode === RepeatMode.QUEUE) repeatMode = 'queue';

        res.json({
            hasQueue: true,
            isPlaying: !queue.paused && queue.playing,
            isPaused: Boolean(queue.paused),
            currentTime: Math.floor(queue.currentTime || 0),
            duration: currentSong?.duration || 0,
            volume: queue.volume,
            repeatMode,
            autoplay: Boolean(queue.autoplay),
            filter: queue.filters.names[0] || null,
            voiceChannel: queue.voiceChannel ? { id: queue.voiceChannel.id, name: queue.voiceChannel.name } : null,
            currentSong,
            queue: upcoming,
        });
    });

    // ─── Resolve playlist (no voice join, for web import preview) ─────
    app.get('/api/guilds/:guildId/playlist/resolve', async (req, res) => {
        const { guildId } = req.params;
        const url = String(req.query.url || '').trim();
        if (!url) return res.status(400).json({ error: 'Query param "url" is required' });

        const guild = client.guilds.cache.get(guildId);
        if (!guild) return res.status(404).json({ error: 'Guild not found' });

        try {
            const playlists = require('../music/playlists');
            const result = await playlists.resolvePlaylist(client.music, guild, url, {
                member: guild.members.me,
            });
            return res.json({
                success: true,
                kind: result.kind,
                name: result.name,
                source: result.source,
                count: result.tracks.length,
                truncated: result.truncated,
                tracks: result.tracks,
            });
        } catch (error) {
            log.error(`API Playlist resolve error: ${error.message}`);
            return res.status(422).json({ success: false, error: error.message });
        }
    });

    // ─── Play / Add to Queue ──────────────────────────────────────────
    app.post('/api/guilds/:guildId/play', async (req, res) => {
        const { guildId } = req.params;
        const { query, voiceChannelId, userId, next = false, skip = false } = req.body;

        if (!query) return res.status(400).json({ error: 'Query is required' });

        const guild = client.guilds.cache.get(guildId);
        if (!guild) return res.status(404).json({ error: 'Guild not found' });

        let channel = null;
        if (voiceChannelId) {
            channel = guild.channels.cache.get(voiceChannelId);
        } else if (userId) {
            const member = guild.members.cache.get(userId) || await guild.members.fetch(userId).catch(() => null);
            channel = member?.voice?.channel;
        }

        const existingQueue = client.music?.distube?.getQueue(guildId);
        if (!channel && existingQueue) {
            channel = existingQueue.voiceChannel;
        }

        if (!channel) {
            channel = guild.channels.cache.find((c) => c.isVoiceBased());
        }

        if (!channel) {
            return res.status(400).json({
                error: 'No target voice channel found. Specify voiceChannelId or connect to voice in Discord.',
            });
        }

        try {
            let member = null;
            if (userId) {
                member = guild.members.cache.get(userId) || await guild.members.fetch(userId).catch(() => null);
            }

            const outcome = await client.music.play({
                voiceChannel: channel,
                member: member || guild.members.me,
                query,
                position: next ? 1 : 0,
                skip: Boolean(skip),
            });

            const resolved = outcome?.resolved;
            const isPlaylist = resolved && Array.isArray(resolved.songs);
            return res.json({
                success: true,
                started: Boolean(outcome?.started),
                position: outcome?.position || 0,
                song: resolved ? formatSong(isPlaylist ? resolved.songs[0] : resolved) : null,
                playlist: isPlaylist
                    ? { name: resolved.name || null, count: resolved.songs.length, truncated: Boolean(outcome?.truncated) }
                    : null,
            });
        } catch (error) {
            log.error(`API Play error: ${error.message}`);
            return res.status(500).json({ error: error.message });
        }
    });

    // ─── Playback Control Actions ─────────────────────────────────────
    app.post('/api/guilds/:guildId/control', async (req, res) => {
        const { guildId } = req.params;
        const { action, value } = req.body;

        const queue = client.music?.distube?.getQueue(guildId);
        if (!queue) {
            return res.status(404).json({ error: 'No active playback queue in this guild' });
        }

        const t = i18n.forGuild(client.guilds.cache.get(guildId));

        try {
            let result = { ok: false, message: 'Unknown action' };

            switch (action) {
                case 'pause':
                    result = await actions.pause(queue, t);
                    break;
                case 'resume':
                    result = await actions.resume(queue, t);
                    break;
                case 'toggle':
                    result = await actions.togglePause(queue, t);
                    break;
                case 'skip':
                    result = await actions.skip(queue, t);
                    break;
                case 'previous':
                    result = await actions.previous(queue, t);
                    break;
                case 'stop':
                    result = await actions.stop(queue, t, client.music);
                    break;
                case 'shuffle':
                    result = await actions.shuffle(queue, t);
                    break;
                case 'autoplay':
                    result = actions.toggleAutoplay(queue, t);
                    break;
                case 'filter': {
                    result = actions.setFilter(queue, t, value);
                    break;
                }
                case 'volume': {
                    result = actions.setVolume(queue, t, value);
                    break;
                }
                case 'seek': {
                    result = await actions.seek(queue, t, Number(value));
                    break;
                }
                case 'loop': {
                    let mode = value;
                    if (value === 'off') mode = RepeatMode.DISABLED;
                    else if (value === 'track') mode = RepeatMode.SONG;
                    else if (value === 'queue') mode = RepeatMode.QUEUE;
                    result = actions.setLoop(queue, t, mode);
                    break;
                }
                default:
                    return res.status(400).json({ error: `Invalid action: ${action}` });
            }

            return res.json({ success: result.ok, message: result.message });
        } catch (error) {
            log.error(`API Control error (${action}): ${error.message}`);
            return res.status(500).json({ error: error.message });
        }
    });

    // ─── Queue Management ─────────────────────────────────────────────
    app.delete('/api/guilds/:guildId/queue/:index', (req, res) => {
        const { guildId, index } = req.params;
        const idx = Number.parseInt(index, 10);

        const queue = client.music?.distube?.getQueue(guildId);
        if (!queue) return res.status(404).json({ error: 'No queue found' });

        if (Number.isNaN(idx) || idx < 1 || idx >= queue.songs.length) {
            return res.status(400).json({ error: 'Invalid song queue index' });
        }

        const removed = queue.songs.splice(idx, 1);
        return res.json({ success: true, removed: removed[0] ? formatSong(removed[0]) : null });
    });

    app.post('/api/guilds/:guildId/queue/move', async (req, res) => {
        const { guildId } = req.params;
        const { from, to } = req.body;
        const queue = client.music?.distube?.getQueue(guildId);
        if (!queue) return res.status(404).json({ error: 'No queue found' });

        const t = i18n.forGuild(client.guilds.cache.get(guildId));
        try {
            const outcome = await actions.safely(
                () => actions.move(queue, t, Number(from), Number(to)), t);
            if (!outcome.ok) return res.status(400).json({ error: outcome.message });
            client.music.changed(queue);
            return res.json({ success: true, message: outcome.message });
        } catch (error) {
            log.error(`API Queue move error: ${error.message}`);
            return res.status(500).json({ error: error.message });
        }
    });

    app.post('/api/guilds/:guildId/queue/jump', async (req, res) => {
        const { guildId } = req.params;
        const { position } = req.body;
        const queue = client.music?.distube?.getQueue(guildId);
        if (!queue) return res.status(404).json({ error: 'No queue found' });

        const t = i18n.forGuild(client.guilds.cache.get(guildId));
        try {
            const outcome = await actions.safely(
                () => actions.jump(queue, t, Number(position)), t);
            if (!outcome.ok) return res.status(400).json({ error: outcome.message });
            return res.json({ success: true, message: outcome.message });
        } catch (error) {
            log.error(`API Queue jump error: ${error.message}`);
            return res.status(500).json({ error: error.message });
        }
    });

    app.delete('/api/guilds/:guildId/queue', (req, res) => {
        const { guildId } = req.params;
        const queue = client.music?.distube?.getQueue(guildId);
        if (!queue) return res.status(404).json({ error: 'No queue found' });

        // Keep current song (index 0) and remove upcoming
        const count = queue.songs.length - 1;
        if (queue.songs.length > 1) {
            queue.songs.splice(1);
        }
        return res.json({ success: true, cleared: count });
    });

    // ─── Start HTTP Listener ──────────────────────────────────────────
    const server = app.listen(config.api.port, '0.0.0.0', () => {
        log.ok(`REST API server listening on http://0.0.0.0:${config.api.port}`);
    });

    return server;
}

module.exports = { startApiServer };
