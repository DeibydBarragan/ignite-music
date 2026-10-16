'use strict';

/**
 * Ultra-fast direct HTTP client for YouTube Innertube API.
 * Bypasses Python/yt-dlp spawn overhead to perform searches,
 * related tracks/mix resolution, and playlist browsing in ~300-800ms.
 */

const USER_AGENT = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36';

function parseDurationFromLabel(label) {
    if (!label) return { duration: 0, formatted: '00:00' };
    let hours = 0, mins = 0, secs = 0;
    const hMatch = label.match(/(\d+)\s*(?:hour|hora|hr)s?/i);
    const mMatch = label.match(/(\d+)\s*(?:minute|minuto|min)s?/i);
    const sMatch = label.match(/(\d+)\s*(?:second|segundo|seg)s?/i);
    if (hMatch) hours = parseInt(hMatch[1], 10);
    if (mMatch) mins = parseInt(mMatch[1], 10);
    if (sMatch) secs = parseInt(sMatch[1], 10);
    const total = hours * 3600 + mins * 60 + secs;
    if (total === 0) return { duration: 0, formatted: '00:00' };
    const formatted = hours > 0
        ? `${hours}:${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`
        : `${mins}:${String(secs).padStart(2, '0')}`;
    return { duration: total, formatted };
}

function parseColonDuration(str) {
    if (!str || typeof str !== 'string') return 0;
    const parts = str.split(':').map(Number);
    if (parts.length === 2 && !isNaN(parts[0]) && !isNaN(parts[1])) {
        return parts[0] * 60 + parts[1];
    }
    if (parts.length === 3 && !isNaN(parts[0]) && !isNaN(parts[1]) && !isNaN(parts[2])) {
        return parts[0] * 3600 + parts[1] * 60 + parts[2];
    }
    return 0;
}

/**
 * 1. Búsqueda de videos en YouTube (Puntos A y D).
 */
async function searchVideosDirect(query, limit = 6, { timeoutMs = 3500 } = {}) {
    const raw = String(query || '').trim();
    if (!raw || raw.length < 2) return [];

    const url = 'https://www.youtube.com/youtubei/v1/search?prettyPrint=false';
    const body = {
        context: {
            client: {
                clientName: 'WEB',
                clientVersion: '2.20240101.00.00',
                hl: 'es',
                gl: 'CO'
            }
        },
        query: raw
    };

    const res = await fetch(url, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'User-Agent': USER_AGENT
        },
        body: JSON.stringify(body),
        signal: AbortSignal.timeout(timeoutMs)
    });

    if (!res.ok) {
        throw new Error(`YouTube Innertube search HTTP ${res.status}`);
    }

    const data = await res.json();
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
 * 2. Recomendaciones de canciones / YouTube Mix (Punto B: Radio y Autoplay).
 */
async function getRelatedMixDirect(videoId, limit = 15, { timeoutMs = 3500 } = {}) {
    if (!videoId || typeof videoId !== 'string') return [];
    const cleanId = videoId.trim();

    const url = 'https://www.youtube.com/youtubei/v1/next?prettyPrint=false';
    const body = {
        context: {
            client: {
                clientName: 'WEB',
                clientVersion: '2.20240101.00.00',
                hl: 'es',
                gl: 'CO'
            }
        },
        videoId: cleanId,
        playlistId: `RD${cleanId}`
    };

    const res = await fetch(url, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'User-Agent': USER_AGENT
        },
        body: JSON.stringify(body),
        signal: AbortSignal.timeout(timeoutMs)
    });

    if (!res.ok) {
        throw new Error(`YouTube Innertube next HTTP ${res.status}`);
    }

    const data = await res.json();
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

        // Filtrar duraciones coherentes (1 a 15 minutos)
        if (durationSec > 0 && (durationSec < 50 || durationSec > 950)) continue;

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

    const url = 'https://www.youtube.com/youtubei/v1/browse?prettyPrint=false';
    const body = {
        context: {
            client: {
                clientName: 'WEB',
                clientVersion: '2.20240101.00.00',
                hl: 'es',
                gl: 'CO'
            }
        },
        browseId: `VL${cleanId}`
    };

    const res = await fetch(url, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'User-Agent': USER_AGENT
        },
        body: JSON.stringify(body),
        signal: AbortSignal.timeout(timeoutMs)
    });

    if (!res.ok) {
        throw new Error(`YouTube Innertube browse HTTP ${res.status}`);
    }

    const data = await res.json();
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
    searchVideosDirect,
    getRelatedMixDirect,
    getPlaylistVideosDirect
};
