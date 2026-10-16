'use strict';

/**
 * Ultra-fast direct HTTP client for YouTube Innertube search.
 * Bypasses the ~600ms Python/yt-dlp spawn overhead to return video
 * results in ~800ms instead of ~2500ms.
 */
async function searchVideosDirect(query, limit = 6) {
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
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36'
        },
        body: JSON.stringify(body),
        signal: AbortSignal.timeout(3500)
    });

    if (!res.ok) {
        throw new Error(`YouTube Innertube HTTP ${res.status}`);
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
            let durationSec = 0;
            if (durationText && durationText !== '00:00' && durationText !== 'EN VIVO') {
                const parts = durationText.split(':').map(Number);
                if (parts.length === 2) durationSec = parts[0] * 60 + parts[1];
                else if (parts.length === 3) durationSec = parts[0] * 3600 + parts[1] * 60 + parts[2];
            }

            const thumbnail = `https://i.ytimg.com/vi/${id}/hqdefault.jpg`;

            videos.push({
                id,
                title,
                artist: channel,
                albumArt: thumbnail,
                coverUrl: thumbnail,
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

module.exports = { searchVideosDirect };
