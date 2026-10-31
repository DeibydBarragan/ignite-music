'use strict';

/**
 * Ultra-fast direct HTTP client for YouTube Innertube API.
 * Bypasses Python/yt-dlp spawn overhead to perform searches,
 * related tracks/mix resolution, and playlist browsing in ~300-800ms.
 */

const config = require('../config');

const {
    USER_AGENT,
    CLIENT_VERSION,
    RELATED_MIN_SECONDS,
    RELATED_MAX_SECONDS,
    parseDurationFromLabel,
    parseColonDuration,
} = require('./innertube-util');

/**
 * POST to a YouTube Innertube endpoint with the shared context/headers and a
 * hard timeout. Throws on non-2xx; the three callers no longer duplicate this.
 */
async function postInnertube(endpoint, payload, timeoutMs = 3500) {
    const res = await fetch(`https://www.youtube.com/youtubei/v1/${endpoint}?prettyPrint=false`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'User-Agent': USER_AGENT,
        },
        body: JSON.stringify({
            context: {
                client: {
                    clientName: 'WEB',
                    clientVersion: CLIENT_VERSION,
                    hl: config.innertube.hl,
                    gl: config.innertube.gl,
                },
            },
            ...payload,
        }),
        signal: AbortSignal.timeout(timeoutMs),
    });
    if (!res.ok) throw new Error(`YouTube Innertube ${endpoint} HTTP ${res.status}`);
    return res.json();
}

/**
 * 1. Búsqueda de videos en YouTube (Puntos A y D).
 */
async function searchVideosDirect(query, limit = 6, { timeoutMs = 3500 } = {}) {
    const raw = String(query || '').trim();
    if (!raw || raw.length < 2) return [];

    const data = /** @type {any} */ (await postInnertube('search', { query: raw }, timeoutMs));
    const sections = data?.contents?.twoColumnSearchResultsRenderer?.primaryContents?.sectionListRenderer?.contents || [];
    const videos = [];

    for (const section of sections) {
        const items = section?.itemSectionRenderer?.contents || [];
        for (const item of items) {
            const v = item.videoRenderer;
            if (!v || !v.videoId) continue;

            const id = v.videoId;
            const title = v.title?.runs?.map((r) => r.text).join('') || v.title?.simpleText || 'Unknown Title';
            const channel = v.ownerText?.runs?.[0]?.text || v.shortBylineText?.runs?.[0]?.text || 'YouTube';
            const isLive = Boolean(
                v.badges?.some((b) => b.metadataBadgeRenderer?.style === 'BADGE_STYLE_TYPE_LIVE_NOW') ||
                v.thumbnailOverlays?.some((o) => o.thumbnailOverlayTimeStatusRenderer?.style === 'LIVE')
            );

            const durationText = v.lengthText?.simpleText || (isLive ? 'EN VIVO' : '00:00');
            const durationSec = isLive ? 0 : parseColonDuration(durationText);
            const thumbnail = `https://i.ytimg.com/vi/${id}/hqdefault.jpg`;

            videos.push({
                id,
                name: title,
                title,
                artist: channel,
                uploader: {
                    name: channel,
                    url: `https://www.youtube.com/watch?v=${id}`
                },
                albumArt: thumbnail,
                coverUrl: thumbnail,
                thumbnail,
                duration: durationSec,
                formattedDuration: isLive ? 'EN VIVO' : durationText,
                source: 'youtube',
                url: `https://www.youtube.com/watch?v=${id}`,
                category: 'hits',
                isLive,
            });

            if (videos.length >= limit) break;
        }
        if (videos.length >= limit) break;
    }

    return videos;
}

/**
 * 2. Búsqueda de playlists en YouTube mediante Innertube (EgIQAw%3D%3D = filtro Playlist).
 */
async function searchPlaylistsDirect(query, limit = 6, { timeoutMs = 3500 } = {}) {
    const raw = String(query || '').trim();
    if (!raw || raw.length < 2) return [];

    const data = /** @type {any} */ (await postInnertube('search', { query: raw, params: 'EgIQAw%3D%3D' }, timeoutMs));
    const sections = data?.contents?.twoColumnSearchResultsRenderer?.primaryContents?.sectionListRenderer?.contents || [];
    const playlists = [];

    for (const section of sections) {
        const items = section?.itemSectionRenderer?.contents || [];
        for (const item of items) {
            if (item.lockupViewModel && (item.lockupViewModel.contentType === 'LOCKUP_CONTENT_TYPE_PLAYLIST' || item.lockupViewModel.contentId?.startsWith('PL'))) {
                const p = item.lockupViewModel;
                const id = p.contentId;
                if (!id) continue;

                const title = p.metadata?.lockupMetadataViewModel?.title?.content || 'Unknown Playlist';
                const uploader = p.metadata?.lockupMetadataViewModel?.metadata?.contentMetadataViewModel?.metadataRows?.[0]?.metadataParts?.[0]?.text?.content || 'YouTube';
                const thumb = p.contentImage?.collectionThumbnailViewModel?.primaryThumbnail?.thumbnailViewModel?.image?.sources?.[0]?.url || `https://i.ytimg.com/vi/${id}/hqdefault.jpg`;
                const badge = p.contentImage?.collectionThumbnailViewModel?.primaryThumbnail?.thumbnailViewModel?.overlays?.[0]?.thumbnailOverlayBadgeViewModel?.thumbnailBadges?.[0]?.thumbnailBadgeViewModel?.text || '';
                const trackCount = parseInt(badge.replace(/[^\d]/g, ''), 10) || 0;

                playlists.push({
                    id,
                    title,
                    name: title,
                    uploader,
                    author: uploader,
                    thumbnail: thumb,
                    cover: thumb,
                    trackCount,
                    url: `https://www.youtube.com/playlist?list=${id}`,
                    source: 'youtube',
                });
            } else if (item.playlistRenderer) {
                const p = item.playlistRenderer;
                const id = p.playlistId;
                if (!id) continue;

                const title = p.title?.simpleText || p.title?.runs?.[0]?.text || 'Unknown Playlist';
                const uploader = p.shortBylineText?.runs?.[0]?.text || 'YouTube';
                const thumb = p.thumbnails?.[0]?.thumbnails?.[0]?.url || p.thumbnailRenderer?.playlistVideoThumbnailRenderer?.thumbnail?.thumbnails?.[0]?.url || `https://i.ytimg.com/vi/${id}/hqdefault.jpg`;
                const trackCount = parseInt(String(p.videoCount || '').replace(/[^\d]/g, ''), 10) || 0;

                playlists.push({
                    id,
                    title,
                    name: title,
                    uploader,
                    author: uploader,
                    thumbnail: thumb,
                    cover: thumb,
                    trackCount,
                    url: `https://www.youtube.com/playlist?list=${id}`,
                    source: 'youtube',
                });
            }

            if (playlists.length >= limit) break;
        }
        if (playlists.length >= limit) break;
    }

    return playlists;
}

/**
 * 3. Recomendaciones de canciones / YouTube Mix (Punto B: Radio y Autoplay).
 */
async function getRelatedMixDirect(videoId, limit = 15, { timeoutMs = 3500 } = {}) {
    if (!videoId || typeof videoId !== 'string') return [];
    const cleanId = videoId.trim();

    const data = /** @type {any} */ (await postInnertube('next', { videoId: cleanId, playlistId: `RD${cleanId}` }, timeoutMs));
    const plItems = data?.contents?.twoColumnWatchNextResults?.playlist?.playlist?.contents || [];
    const related = [];
    const seen = new Set([cleanId]);

    // Intentar primero con la lista de YouTube Mix
    for (const item of plItems) {
        const v = item.playlistPanelVideoRenderer;
        if (!v || !v.videoId || seen.has(v.videoId)) continue;
        seen.add(v.videoId);

        const durationText = v.lengthText?.simpleText || '00:00';
        const durationSec = parseColonDuration(durationText);

        if (durationSec > 0 && (durationSec < RELATED_MIN_SECONDS || durationSec > RELATED_MAX_SECONDS)) continue;

        const title = v.title?.simpleText || v.title?.runs?.map((r) => r.text).join('') || 'Unknown Title';
        const channel = v.shortBylineText?.runs?.[0]?.text || 'YouTube';
        const thumbnail = `https://i.ytimg.com/vi/${v.videoId}/hqdefault.jpg`;

        related.push({
            id: v.videoId,
            name: title,
            title,
            artist: channel,
            uploader: {
                name: channel,
                url: `https://www.youtube.com/watch?v=${v.videoId}`
            },
            albumArt: thumbnail,
            coverUrl: thumbnail,
            thumbnail,
            duration: durationSec,
            formattedDuration: durationText,
            source: 'youtube',
            url: `https://www.youtube.com/watch?v=${v.videoId}`,
            category: 'hits',
            isLive: false,
        });

        if (related.length >= limit) break;
    }

    return related;
}

/**
 * 3. Sincronización e Importación de Playlists de YouTube (Punto C).
 */
async function getPlaylistVideosDirect(playlistId, limit = 200, { timeoutMs = 5000 } = {}) {
    if (!playlistId || typeof playlistId !== 'string') return null;
    const cleanId = playlistId.trim().replace(/^VL/, '');

    const data = /** @type {any} */ (await postInnertube('browse', { browseId: `VL${cleanId}` }, timeoutMs));
    const plTitle = data?.metadata?.playlistMetadataRenderer?.title || 'YouTube Playlist';
    const rawItems = data?.contents?.twoColumnBrowseResultsRenderer?.tabs?.[0]?.tabRenderer?.content?.sectionListRenderer?.contents?.[0]?.itemSectionRenderer?.contents || [];

    const tracks = [];
    for (const item of rawItems) {
        if (item.lockupViewModel) {
            const v = item.lockupViewModel;
            const id = v.contentId;
            if (!id) continue;

            const title = v.metadata?.lockupMetadataViewModel?.title?.content || 'Unknown Title';
            const artist = v.metadata?.lockupMetadataViewModel?.metadata?.contentMetadataViewModel?.metadataRows?.[0]?.metadataParts?.[0]?.text?.content || 'YouTube';
            const label = v.rendererContext?.accessibilityContext?.label || '';
            const { duration, formatted } = parseDurationFromLabel(label);
            const thumbnail = `https://i.ytimg.com/vi/${id}/hqdefault.jpg`;

            tracks.push({
                id,
                name: title,
                title,
                artist,
                uploader: { name: artist },
                albumArt: thumbnail,
                coverUrl: thumbnail,
                thumbnail,
                duration,
                formattedDuration: formatted,
                source: 'youtube',
                url: `https://www.youtube.com/watch?v=${id}`,
                category: 'hits',
                isLive: false,
            });
        } else if (item.playlistVideoRenderer) {
            const v = item.playlistVideoRenderer;
            const id = v.videoId;
            if (!id) continue;

            const title = v.title?.runs?.map((r) => r.text).join('') || v.title?.simpleText || 'Unknown Title';
            const artist = v.shortBylineText?.runs?.[0]?.text || 'YouTube';
            const formatted = v.lengthText?.simpleText || '00:00';
            const duration = parseColonDuration(formatted);
            const thumbnail = `https://i.ytimg.com/vi/${id}/hqdefault.jpg`;

            tracks.push({
                id,
                name: title,
                title,
                artist,
                uploader: { name: artist },
                albumArt: thumbnail,
                coverUrl: thumbnail,
                thumbnail,
                duration,
                formattedDuration: formatted,
                source: 'youtube',
                url: `https://www.youtube.com/watch?v=${id}`,
                category: 'hits',
                isLive: false,
            });
        }

        if (tracks.length >= limit) break;
    }

    return {
        title: plTitle,
        tracks
    };
}

module.exports = {
    postInnertube,
    searchVideosDirect,
    searchPlaylistsDirect,
    getRelatedMixDirect,
    getPlaylistVideosDirect,
    RELATED_MIN_SECONDS,
    RELATED_MAX_SECONDS
};
