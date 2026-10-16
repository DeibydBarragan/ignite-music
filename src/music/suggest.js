'use strict';

const { TTLCache } = require('../core/cache');
const { searchVideosDirect } = require('./innertube');

const suggestionCache = new TTLCache({ ttlMs: 10 * 60_000, maxSize: 2000 });
const videoChoiceCache = new TTLCache({ ttlMs: 10 * 60_000, maxSize: 1000 });

/**
 * YouTube search suggestions for /play autocomplete fallback.
 */
async function suggestions(query, { timeoutMs = 1200 } = {}) {
    const value = String(query || '').trim();
    if (value.length < 2 || /^https?:\/\//i.test(value)) return [];
    const key = value.toLowerCase();

    return suggestionCache.wrap(key, async () => {
        const url = `https://suggestqueries.google.com/complete/search?client=firefox&ds=yt&q=${encodeURIComponent(value)}`;
        const response = await fetch(url, { signal: AbortSignal.timeout(timeoutMs) });
        if (!response.ok) return [];
        const body = JSON.parse(Buffer.from(await response.arrayBuffer()).toString('utf8'));
        return Array.isArray(body?.[1]) ? body[1].filter((item) => typeof item === 'string').slice(0, 10) : [];
    }).catch(() => []);
}

/**
 * Autocomplete choices:
 * - If length >= 3: searches real YouTube videos via Innertube (with 1.4s strict timeout).
 *   Returns exact video title + duration, with direct YouTube URL as value.
 * - If length < 3, timeout, or error: falls back to Google search suggestions.
 */
async function choices(query) {
    const value = String(query || '').trim();
    // Links and long text are submitted exactly as typed; a choice would truncate them to 100 characters.
    if (!value || value.length > 100 || /^https?:\/\//i.test(value)) return [];

    const key = value.toLowerCase();

    // Fast-path: If user typed 3+ chars, try direct video search
    if (value.length >= 3) {
        try {
            const videos = await videoChoiceCache.wrap(key, async () => {
                return searchVideosDirect(value, 7, { timeoutMs: 1400 });
            });

            if (Array.isArray(videos) && videos.length > 0) {
                const results = [{ name: `🔍 ${value}`.slice(0, 100), value }];
                const seen = new Set();
                for (const v of videos) {
                    if (!v.id || seen.has(v.id)) continue;
                    seen.add(v.id);
                    const dur = v.formattedDuration ? ` [${v.formattedDuration}]` : '';
                    const titleWithDur = `${v.title}${dur}`.slice(0, 100);
                    results.push({
                        name: titleWithDur,
                        value: v.url
                    });
                }
                return results.slice(0, 25);
            }
        } catch {
            // Innertube timed out or errored: transparently fallback to quick suggestions below
        }
    }

    // Fallback: Google search text suggestions
    const items = await suggestions(value);
    const seen = new Set([value.toLowerCase()]);
    const rest = items
        .filter((item) => item.length <= 100 && !seen.has(item.toLowerCase()) && seen.add(item.toLowerCase()))
        .map((item) => ({ name: item, value: item }));
    return [{ name: value, value }, ...rest].slice(0, 25);
}

module.exports = { suggestions, choices };
