'use strict';

/**
 * Shared constants (Phase 2.3): previously duplicated across
 * api/contracts.js, music/top-songs.js, core/i18n.js and core/format.js.
 */

const FALLBACK_COVER = 'https://images.unsplash.com/photo-1470225620780-dba8ba36b745?w=300&q=80';

/**
 * Superset emoji pattern (pictographic + flags + skin tones + modifiers),
 * used by i18n (strip emoji from translations) and format (sanitize text).
 * Only pass it to String methods such as replace(), never to test().
 */
const EMOJI = /[\p{Extended_Pictographic}\p{Regional_Indicator}\u{1F3FB}-\u{1F3FF}\u20E3\uFE0E\uFE0F\u200D]/gu;

module.exports = { FALLBACK_COVER, EMOJI };
