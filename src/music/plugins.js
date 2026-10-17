'use strict';

const { ExtractorPlugin, Playlist, Song, DisTubeError } = /** @type {any} */ (require('distube'));
const config = require('../config');
const ytdlp = require('./ytdlp');
const { getRelay } = require('./relay');
const { TTLCache } = require('../core/cache');
const { isBotCheck } = require('../core/youtube-guards');
const { setTimeout: sleep } = require('node:timers/promises');
const log = require('../core/logger').createLogger('extractor');
const { searchVideosDirect, getRelatedMixDirect } = require('./innertube');
const {
    AUDIO_FORMAT,
    INNERTUBE_TIMEOUT_MS,
    VIDEO_ID,
    YtDlpBase,
    isLyricSong,
    isLyricTitle,
    guildIdOf,
    streamTtl,
    parseYouTubeUrl,
    watchUrl,
    directSongInfo,
    isPlayableRelated,
    splitYouTubeTitle,
    youtubeInfo,
    relayStream,
} = require('./plugins-base');
const { GenericPlugin, RelayedDirectLinkPlugin } = require('./plugins-generic');

class YouTubePlugin extends ExtractorPlugin {
    /**
     * @param {{ runner?: any, relay?: any, direct?: { search?: any, related?: any } }} [options]
     */
    constructor({ runner = ytdlp.runJson, relay = getRelay(), direct = {} } = {}) {
        super();
        this.base = new YtDlpBase(runner, relay);
        this.direct = {
            search: direct.search || searchVideosDirect,
            related: direct.related || getRelatedMixDirect,
        };
        this.searches = new TTLCache({ ttlMs: 30 * 60_000, maxSize: 1000 });
        this.lastSearchError = null;
        this.lastSearchErrorAt = 0;
    }

    /**
     * Innertube-first flow shared by search() and getRelatedSongs(): a single
     * direct call with a short timeout, then yt-dlp takes over on error or no
     * usable entries (null tells the caller to fall back).
     */
    async directEntries(load, filter, describe) {
        try {
            const result = await load();
            const entries = (Array.isArray(result) ? result : []).filter(filter);
            if (entries.length > 0) return entries;
        } catch (error) {
            log.warn(`Innertube ${describe} failed, falling back to yt-dlp: ${error.message}`);
        }
        return null;
    }

    get run() {
        return this.base.run;
    }

    validate(url) {
        return parseYouTubeUrl(url) !== null;
    }

    async resolve(url, options = {}) {
        const parsed = parseYouTubeUrl(url);
        if (!parsed) throw new DisTubeError('NOT_SUPPORTED_URL');
        const guildId = guildIdOf(options);
        const proxy = ytdlp.proxyFor(guildId);

        if (parsed.type === 'playlist') {
            const info = await this.run(`https://www.youtube.com/playlist?list=${parsed.id}`, [
                '--flat-playlist', '--playlist-end', String(config.bot.maxPlaylistSize),
            ], { proxy });
            const songs = (info.entries || [])
                .filter((entry) => entry?.id && VIDEO_ID.test(entry.id) && entry.title !== '[Private video]' && entry.title !== '[Deleted video]')
                .map((entry) => new Song({ ...youtubeInfo(entry), plugin: this }, options));
            if (!songs.length) throw new DisTubeError('EMPTY_PLAYLIST');
            return new Playlist({
                source: 'youtube',
                songs,
                id: parsed.id,
                name: info.title || 'YouTube playlist',
                url: `https://www.youtube.com/playlist?list=${parsed.id}`,
                thumbnail: songs[0].thumbnail,
            }, options);
        }

        try {
            const info = await this.run(watchUrl(parsed.id), ['--no-playlist', '-f', AUDIO_FORMAT], { proxy });
            const song = new Song({ ...youtubeInfo(info), plugin: this }, options);
            this.base.rememberInfo(guildId, song.url, info);
            return song;
        } catch (error) {
            log.warn(`YouTube resolve failed for ${url}: ${error.message}`);
            throw error;
        }
    }

    /**
     * Used by DisTube for text queries and for Spotify tracks (which are played from YouTube).
     */
    async searchSong(query, options = {}) {
        const guildId = guildIdOf(options);
        let info;
        try {
            info = await this.searchInfo(query, guildId);
        } catch (error) {
            this.lastSearchError = error;
            this.lastSearchErrorAt = Date.now();
            if (isBotCheck(error)) {
                log.warn(`YouTube search challenged for "${query}", retrying once in 8s`);
                await sleep(8000);
                try {
                    info = await this.searchInfo(query, guildId);
                } catch (retryError) {
                    this.lastSearchError = retryError;
                    this.lastSearchErrorAt = Date.now();
                    log.warn(`YouTube search failed for "${query}": ${retryError.message}`);
                    throw retryError;
                }
            } else {
                log.warn(`YouTube search failed for "${query}": ${error.message}`);
                throw error;
            }
        }
        if (!info) return null;
        const song = new Song({ ...youtubeInfo(info), plugin: this }, options);
        this.base.rememberInfo(guildId, song.url, info);
        return song;
    }

    searchInfo(query, guildId, { priority = 'high' } = {}) {
        const proxy = ytdlp.proxyFor(guildId);
        return this.searches.wrap(`${proxy || 'direct'}|${query.toLowerCase()}`, async () => {
            const result = await this.run(`ytsearch1:${query}`, ['-f', AUDIO_FORMAT], { proxy, priority });
            const info = result?.entries ? result.entries.find(Boolean) : result;
            return info?.id ? info : null;
        });
    }

    /**
     * Several results for /search (metadata only, ultra-fast direct Innertube with yt-dlp fallback).
     */
    async search(query, limit = 5, guildId = null) {
        const direct = await this.directEntries(
            () => this.direct.search(query, limit, { timeoutMs: INNERTUBE_TIMEOUT_MS }),
            (entry) => entry?.id && entry.url,
            `search for "${query}"`
        );
        if (direct) return direct.map(directSongInfo);

        const result = await this.run(`ytsearch${limit}:${query}`, ['--flat-playlist'], { proxy: ytdlp.proxyFor(guildId) });
        return (result?.entries || []).filter((entry) => entry?.id && VIDEO_ID.test(entry.id)).map(youtubeInfo);
    }

    async getStreamURL(song) {
        return this.base.streamUrl(song);
    }

    async prefetch(song) {
        await this.base.streamInfo(song, { priority: 'low' });
    }

    async prefetchQuery(query, guildId) {
        const info = await this.searchInfo(query, guildId, { priority: 'low' });
        if (info) this.base.rememberInfo(guildId, watchUrl(info.id), info);
    }

    async getRelatedSongs(song) {
        if (!VIDEO_ID.test(song?.id || '')) return [];
        const guildId = guildIdOf(song);

        const direct = await this.directEntries(
            () => this.direct.related(song.id, 15, { timeoutMs: INNERTUBE_TIMEOUT_MS }),
            (entry) => isPlayableRelated(entry, song.id),
            `related songs for video ${song.id}`
        );
        if (direct) {
            return direct.map((entry) => new Song(
                { ...directSongInfo(entry), plugin: this },
                { member: song.member, metadata: song.metadata }
            ));
        }

        const result = await this.run(`${watchUrl(song.id)}&list=RD${song.id}`, [
            '--flat-playlist', '--playlist-end', '15',
        ], { proxy: ytdlp.proxyFor(guildId), priority: 'low' });
        return (result?.entries || [])
            .filter((entry) => isPlayableRelated(entry, song.id))
            .map((entry) => new Song({ ...youtubeInfo(entry), plugin: this }, { member: song.member, metadata: song.metadata }));
    }
}

module.exports = {
    YouTubePlugin,
    GenericPlugin,
    RelayedDirectLinkPlugin,
    parseYouTubeUrl,
    youtubeInfo,
    splitYouTubeTitle,
    isLyricTitle,
    isLyricSong,
    streamTtl,
    relayStream,
    AUDIO_FORMAT,
};
