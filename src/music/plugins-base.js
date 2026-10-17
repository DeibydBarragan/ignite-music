'use strict';

const { DisTubeError } = /** @type {any} */ (require('distube'));
const ytdlp = require('./ytdlp');
const { TTLCache } = require('../core/cache');
const { RELATED_MIN_SECONDS, RELATED_MAX_SECONDS } = require('./innertube');

const AUDIO_FORMAT = 'bestaudio[acodec=opus]/bestaudio/best';
const INNERTUBE_TIMEOUT_MS = 3000;
const YOUTUBE_HOSTS = new Set([
    'youtube.com', 'www.youtube.com', 'm.youtube.com', 'music.youtube.com', 'youtu.be', 'www.youtu.be',
    'youtube-nocookie.com', 'www.youtube-nocookie.com',
]);
const VIDEO_ID = /^[\w-]{11}$/;

const lyricUrls = new TTLCache({ ttlMs: 24 * 3600_000, maxSize: 5000 });

function isLyricTitle(rawTitle) {
    return /\blyrics?\b/i.test(String(rawTitle || ''));
}

function isLyricSong(url) {
    if (!url) return false;
    try {
        return lyricUrls.get(String(url)) === true;
    } catch {
        return false;
    }
}

const guildIdOf = (holder) => holder?.metadata?.guildId || holder?.member?.guild?.id || null;

function streamTtl(url) {
    try {
        const expire = Number(new URL(url).searchParams.get('expire'));
        if (expire > 0) return Math.max(60_000, Math.min(4 * 3600_000, expire * 1000 - Date.now() - 30 * 60_000));
    } catch {
        // not a URL with an expiry
    }
    return 3600_000;
}

function parseYouTubeUrl(input) {
    let url;
    try {
        url = new URL(input);
    } catch {
        return null;
    }
    if (!YOUTUBE_HOSTS.has(url.hostname.toLowerCase())) return null;

    const list = url.searchParams.get('list');
    let videoId = url.searchParams.get('v');
    if (url.hostname.endsWith('youtu.be')) videoId = url.pathname.slice(1).split('/')[0];
    const pathMatch = url.pathname.match(/^\/(?:shorts|live|embed|v)\/([\w-]{11})/);
    if (pathMatch) videoId = pathMatch[1];

    const hasVideo = Boolean(videoId && VIDEO_ID.test(videoId));
    const isMix = Boolean(list && list.startsWith('RD'));
    if (list && /^[\w-]+$/.test(list) && !(isMix && hasVideo)) return { type: 'playlist', id: list };
    if (hasVideo) return { type: 'video', id: videoId };
    return null;
}

const watchUrl = (id) => `https://www.youtube.com/watch?v=${id}`;
const thumbnailFor = (id) => `https://i.ytimg.com/vi/${id}/hqdefault.jpg`;

function directSongInfo(entry) {
    const { title, artist } = splitYouTubeTitle(entry.title, entry.artist);
    return {
        source: 'youtube',
        playFromSource: true,
        id: entry.id,
        name: title || entry.title,
        url: entry.url,
        isLive: Boolean(entry.isLive),
        duration: entry.duration || 0,
        thumbnail: entry.thumbnail || thumbnailFor(entry.id),
        uploader: {
            name: artist || entry.artist,
            url: entry.uploader?.url,
        },
    };
}

function isPlayableRelated(entry, currentId) {
    return Boolean(
        entry?.id &&
        entry.id !== currentId &&
        VIDEO_ID.test(entry.id) &&
        (!entry.duration || (entry.duration >= RELATED_MIN_SECONDS && entry.duration <= RELATED_MAX_SECONDS))
    );
}

const OFFICIAL_SUFFIX = /\s*[\(\[]\s*(official\s*(music\s*)?video|official\s*audio|video\s*oficial|audio\s*oficial|official\s*lyric\s*video|lyric\s*video|lyrics?|visualizer)\s*[\)\]]\s*/gi;

function cleanTitleSuffix(title) {
    return String(title || '')
        .replace(OFFICIAL_SUFFIX, ' ')
        .replace(/\s{2,}/g, ' ')
        .trim();
}

function normName(value) {
    return String(value || '')
        .toLowerCase()
        .normalize('NFD')
        .replace(/[\u0300-\u036f]/g, '')
        .replace(/vevo$/, '')
        .replace(/[^a-z0-9 ]/g, '')
        .replace(/\s+/g, ' ')
        .trim();
}

function splitYouTubeTitle(rawTitle, uploader) {
    const cleaned = cleanTitleSuffix(rawTitle);
    const channel = String(uploader || '').trim();

    const topic = channel.match(/^(.*?)\s*-\s*topic$/i);
    if (topic && topic[1].trim()) {
        return { title: cleaned || String(rawTitle || '').trim(), artist: topic[1].trim() };
    }

    const dash = cleaned.indexOf(' - ');
    if (dash > 0 && channel) {
        const left = cleaned.slice(0, dash).trim();
        const right = cleaned.slice(dash + 3).trim();
        const leftNorm = normName(left);
        const channelNorm = normName(channel);
        if (
            right &&
            leftNorm &&
            channelNorm &&
            (leftNorm.includes(channelNorm) || channelNorm.includes(leftNorm))
        ) {
            return { title: right, artist: left };
        }
    }

    return { title: cleaned || String(rawTitle || '').trim(), artist: channel };
}

function youtubeInfo(info) {
    const id = info.id;
    const isLive = Boolean(info.is_live || info.live_status === 'is_live');
    const uploaderName = info.uploader || info.channel || undefined;
    const rawName = info.title || info.fulltitle || id;
    const { title, artist } = splitYouTubeTitle(rawName, uploaderName);
    const url = VIDEO_ID.test(id || '') ? watchUrl(id) : info.webpage_url || info.url;
    if (url) lyricUrls.set(String(url), isLyricTitle(rawName));
    return {
        source: 'youtube',
        playFromSource: true,
        id,
        name: title,
        url,
        isLive,
        duration: isLive ? 0 : Number(info.duration) || 0,
        thumbnail: VIDEO_ID.test(id || '') ? thumbnailFor(id) : info.thumbnail,
        uploader: {
            name: artist || uploaderName,
            url: info.uploader_url || info.channel_url || undefined,
        },
        views: info.view_count,
        likes: info.like_count,
        ageRestricted: Number(info.age_limit) >= 18,
    };
}

async function relayStream(relay, source) {
    const url = await relay.register(source);
    return relay.warm(url);
}

class YtDlpBase {
    constructor(runner, relay) {
        this.run = runner;
        this.relay = relay;
        this.infos = new TTLCache({ ttlMs: 3600_000, maxSize: 500 });
    }

    infoKey(guildId, url) {
        return `${ytdlp.proxyFor(guildId) || 'direct'}|${url}`;
    }

    rememberInfo(guildId, pageUrl, info) {
        if (info?.url && info.format_id && !info.entries) this.infos.set(this.infoKey(guildId, pageUrl), info, streamTtl(info.url));
    }

    async streamInfo(song, { priority = 'high' } = {}) {
        if (!song?.url) throw new DisTubeError('INVALID_SONG', 'Cannot get a stream without a song URL.');
        const guildId = guildIdOf(song);
        const key = this.infoKey(guildId, song.url);
        const info = await this.infos.wrap(key, async () => {
            const result = await this.run(song.url, ['--no-playlist', '-f', AUDIO_FORMAT], { proxy: ytdlp.proxyFor(guildId), priority });
            if (!result?.url) throw new DisTubeError('NO_STREAM_URL', song.name || song.url);
            return result;
        });
        this.infos.set(key, info, streamTtl(info.url));
        return info;
    }

    async streamUrl(song) {
        const info = await this.streamInfo(song);
        return relayStream(this.relay, { info, proxy: ytdlp.proxyFor(guildIdOf(song)) });
    }
}

module.exports = {
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
    thumbnailFor,
    directSongInfo,
    isPlayableRelated,
    splitYouTubeTitle,
    youtubeInfo,
    relayStream,
};
