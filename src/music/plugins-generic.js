'use strict';

const { PlayableExtractorPlugin, Playlist, Song, DisTubeError } = /** @type {any} */ (require('distube'));
const { DirectLinkPlugin } = require('@distube/direct-link');
const config = require('../config');
const ytdlp = require('./ytdlp');
const { getRelay } = require('./relay');
const { YtDlpBase, guildIdOf, relayStream } = require('./plugins-base');

/**
 * Fallback for every other site yt-dlp supports (Bandcamp, Vimeo, Twitch, ...). Must be the last plugin.
 */
class GenericPlugin extends PlayableExtractorPlugin {
    constructor({ runner = ytdlp.runJson, relay = getRelay() } = {}) {
        super();
        this.base = new YtDlpBase(runner, relay);
    }

    validate(url) {
        try {
            return ['http:', 'https:'].includes(new URL(url).protocol);
        } catch {
            return false;
        }
    }

    async resolve(url, options = {}) {
        const guildId = guildIdOf(options);
        const info = await this.base.run(url, [
            '--flat-playlist', '--playlist-end', String(config.bot.maxPlaylistSize), '-f', 'bestaudio/best',
        ], { proxy: ytdlp.proxyFor(guildId) });

        const toSong = (entry) => new Song({
            source: entry.extractor_key || entry.ie_key || entry.extractor || 'generic',
            playFromSource: true,
            id: String(entry.id ?? entry.url),
            name: entry.title || entry.id || entry.url,
            url: entry.webpage_url || entry.url,
            isLive: Boolean(entry.is_live),
            duration: entry.is_live ? 0 : Number(entry.duration) || 0,
            thumbnail: entry.thumbnail || entry.thumbnails?.at(-1)?.url,
            uploader: { name: entry.uploader || entry.channel, url: entry.uploader_url || entry.channel_url },
            ageRestricted: Number(entry.age_limit) >= 18,
            plugin: this,
        }, options);

        if (Array.isArray(info.entries)) {
            const songs = info.entries.filter((entry) => entry && (entry.webpage_url || entry.url)).map(toSong);
            if (!songs.length) throw new DisTubeError('EMPTY_PLAYLIST');
            return new Playlist({
                source: info.extractor_key || 'generic',
                songs,
                id: String(info.id ?? url),
                name: info.title || url,
                url: info.webpage_url || url,
                thumbnail: info.thumbnail || songs[0].thumbnail,
            }, options);
        }

        const song = toSong(info);
        this.base.rememberInfo(guildId, song.url, info);
        return song;
    }

    getStreamURL(song) {
        return this.base.streamUrl(song);
    }

    async prefetch(song) {
        await this.base.streamInfo(song, { priority: 'low' });
    }

    getRelatedSongs() {
        return [];
    }
}

/**
 * Direct links play through the relay, so ffmpeg never opens network connections itself.
 */
const DirectLinkBase = /** @type {any} */ (DirectLinkPlugin);

class RelayedDirectLinkPlugin extends DirectLinkBase {
    constructor({ relay = getRelay() } = {}) {
        super();
        this.relay = relay;
    }

    async getStreamURL(song) {
        const url = super.getStreamURL(song);
        return relayStream(this.relay, { url, proxy: ytdlp.proxyFor(guildIdOf(song)) });
    }
}

module.exports = { GenericPlugin, RelayedDirectLinkPlugin };
