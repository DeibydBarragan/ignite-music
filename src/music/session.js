'use strict';

const path = require('node:path');
const { Song } = require('distube');
const config = require('../config');
const { JsonStore } = require('../core/store');

const MAX_AGE_MS = 30 * 60_000;

/**
 * Saves each guild's queue so playback resumes where it stopped after a restart or crash.
 */
class SessionStore {
    constructor(plugins, dir = path.join(config.dataDir, 'sessions')) {
        this.store = new JsonStore(dir);
        this.plugins = plugins; // { key: pluginInstance }
    }

    pluginKey(plugin) {
        return Object.keys(this.plugins).find((key) => this.plugins[key] === plugin) || null;
    }

    serializeSong(song) {
        const user = song.metadata?.requesterUser || song.member?.user || song.user;
        const isBot = Boolean(song.member?.user?.bot || user?.bot);
        return {
            plugin: this.pluginKey(song.plugin),
            source: song.source,
            playFromSource: Boolean(song.stream?.playFromSource),
            id: song.id,
            name: song.name,
            url: song.url,
            duration: song.duration,
            isLive: Boolean(song.isLive),
            thumbnail: song.thumbnail,
            uploader: { name: song.uploader?.name, url: song.uploader?.url },
            ageRestricted: Boolean(song.ageRestricted),
            requesterId: song.metadata?.requesterId || (isBot ? null : user?.id),
            requesterUser: song.metadata?.requesterUser || (user ? {
                id: user.id,
                username: song.member?.displayName || user.globalName || user.username,
                avatar: typeof (song.member?.displayAvatarURL || user?.displayAvatarURL) === 'function'
                    ? (song.member?.displayAvatarURL?.() || user?.displayAvatarURL?.())
                    : (user.avatar || null),
                bot: isBot,
            } : null),
            autoplay: isBot,
        };
    }

    serialize(queue) {
        return {
            voiceChannelId: queue.voiceChannel?.id || queue.voice?.channelId || null,
            textChannelId: queue.textChannel?.id || null,
            songs: queue.songs.slice(0, config.bot.maxQueueSize).map((song) => this.serializeSong(song)).filter((song) => song.plugin && song.url),
            position: Math.floor(queue.currentTime || 0),
            volume: queue.volume,
            repeatMode: queue.repeatMode,
            autoplay: queue.autoplay,
            filters: queue.filters.names,
            paused: queue.paused,
            savedAt: Date.now(),
        };
    }

    save(queue) {
        if (!queue?.songs?.length) return this.store.delete(queue.id);
        return this.store.set(queue.id, this.serialize(queue));
    }

    remove(guildId) {
        return this.store.delete(guildId);
    }

    load(guildId) {
        const data = this.store.get(guildId);
        if (!data || !Array.isArray(data.songs) || !data.songs.length) return null;
        if (Date.now() - (data.savedAt || 0) > MAX_AGE_MS) return null;
        return data;
    }

    guildIds() {
        return this.store.keys();
    }

    /**
     * Rebuild DisTube songs without any network request.
     */
    buildSongs(data, guild) {
        const me = guild.members.me;
        return data.songs
            .filter((item) => this.plugins[item.plugin])
            .map((item) => {
                const member = item.autoplay ? me : guild.members.cache.get(item.requesterId);
                return new Song({
                    plugin: this.plugins[item.plugin],
                    source: item.source || item.plugin,
                    playFromSource: item.playFromSource,
                    id: item.id,
                    name: item.name,
                    url: item.url,
                    duration: item.duration,
                    isLive: item.isLive,
                    thumbnail: item.thumbnail,
                    uploader: item.uploader,
                    ageRestricted: item.ageRestricted,
                }, { member: member || undefined, metadata: { requesterId: item.requesterId, requesterUser: item.requesterUser, guildId: guild.id } });
            });
    }

    drain() {
        return this.store.drain();
    }
}

module.exports = { SessionStore, MAX_AGE_MS };
