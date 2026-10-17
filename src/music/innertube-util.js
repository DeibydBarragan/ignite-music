'use strict';

const USER_AGENT = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36';
const CLIENT_VERSION = '2.20240101.00.00';

const RELATED_MIN_SECONDS = 60;
const RELATED_MAX_SECONDS = 900;

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

module.exports = {
    USER_AGENT,
    CLIENT_VERSION,
    RELATED_MIN_SECONDS,
    RELATED_MAX_SECONDS,
    parseDurationFromLabel,
    parseColonDuration,
};
