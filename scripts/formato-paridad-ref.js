'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { formatDuration, parseTime, progressBar, truncate, escapeMarkdown, safeUrl, linkLabel, link, chunk } = require('../src/core/format');

function formatUptime(ms) {
    const minutes = Math.floor(ms / 60_000);
    const days = Math.floor(minutes / 1440);
    const hours = Math.floor((minutes % 1440) / 60);
    return [days ? `${days}d` : null, hours ? `${hours}h` : null, `${minutes % 60}m`].filter(Boolean).join(' ');
}

const u = 'https://www.youtube.com/watch?v=rFZHOHl-L8A';

const fixture = {
    formatDuration: [
        { input: 0, output: formatDuration(0) },
        { input: 65, output: formatDuration(65) },
        { input: 3661, output: formatDuration(3661) },
        { input: -5, output: formatDuration(-5) },
    ],
    parseTime: [
        { input: '90', output: parseTime('90') },
        { input: '1:30', output: parseTime('1:30') },
        { input: '1:02:03', output: parseTime('1:02:03') },
        { input: '1:60', output: parseTime('1:60') },
        { input: 'abc', output: parseTime('abc') },
        { input: '', output: parseTime('') },
        { input: '-5', output: parseTime('-5') },
        { input: '0', output: parseTime('0') },
        { input: '0:00', output: parseTime('0:00') },
        { input: '0:59', output: parseTime('0:59') },
        { input: '2:00:00', output: parseTime('2:00:00') },
    ],
    progressBar: [
        { current: 0, total: 100, size: 12, output: progressBar(0, 100, 12) },
        { current: 50, total: 100, size: 12, output: progressBar(50, 100, 12) },
        { current: 100, total: 100, size: 12, output: progressBar(100, 100, 12) },
        { current: 150, total: 100, size: 12, output: progressBar(150, 100, 12) },
        { current: 10, total: 0, size: 12, output: progressBar(10, 0, 12) },
        { current: 42, total: 181, size: 16, output: progressBar(42, 181, 16) },
    ],
    truncate: [
        { text: 'abcdef', max: 4, output: truncate('abcdef', 4) },
        { text: 'abc', max: 4, output: truncate('abc', 4) },
        { text: 'hello world', max: 5, output: truncate('hello world', 5) },
        { text: 'hi', max: 10, output: truncate('hi', 10) },
    ],
    escapeMarkdown: [
        { text: 'a*b_[c](d)', output: escapeMarkdown('a*b_[c](d)') },
        { text: 'test_string~with|special>chars\\and`quotes', output: escapeMarkdown('test_string~with|special>chars\\and`quotes') },
    ],
    safeUrl: [
        { url: 'https://x.y/a)b', output: safeUrl('https://x.y/a)b') },
        { url: 'javascript:alert(1)', output: safeUrl('javascript:alert(1)') },
        { url: 'ftp://x', output: safeUrl('ftp://x') },
        { url: 'http://example.com/test?a=1&b=2', output: safeUrl('http://example.com/test?a=1&b=2') },
    ],
    linkLabel: [
        { title: 'Title', max: 80, output: linkLabel('Title', 80) },
        { title: 'lofi hip hop radio 📚 beats to relax/study to 2026-10-09 05:36', max: 80, output: linkLabel('lofi hip hop radio 📚 beats to relax/study to 2026-10-09 05:36', 80) },
        { title: '[MV] Song (Live)', max: 80, output: linkLabel('[MV] Song (Live)', 80) },
        { title: 'a\nb\u200bc', max: 80, output: linkLabel('a\nb\u200bc', 80) },
        { title: '👨‍👩‍👧 🇹🇷 Türkçe şarkı', max: 80, output: linkLabel('👨‍👩‍👧 🇹🇷 Türkçe şarkı', 80) },
        { title: 'see https://example.com', max: 80, output: linkLabel('see https://example.com', 80) },
        { title: '🔥🔥', max: 80, output: linkLabel('🔥🔥', 80) },
        { title: null, max: 80, output: linkLabel(null, 80) },
    ],
    link: [
        { title: 'Title', url: 'https://x.y/a)b', max: 80, output: link('Title', 'https://x.y/a)b', 80) },
        { title: 'Title', url: 'javascript:alert(1)', max: 80, output: link('Title', 'javascript:alert(1)', 80) },
        { title: 'lofi hip hop radio 📚 beats to relax/study to 2026-10-09 05:36', url: u, max: 80, output: link('lofi hip hop radio 📚 beats to relax/study to 2026-10-09 05:36', u, 80) },
        { title: '[MV] Song (Live)', url: u, max: 80, output: link('[MV] Song (Live)', u, 80) },
        { title: 'a\nb\u200bc', url: u, max: 80, output: link('a\nb\u200bc', u, 80) },
        { title: '👨‍👩‍👧 🇹🇷 Türkçe şarkı', url: u, max: 80, output: link('👨‍👩‍👧 🇹🇷 Türkçe şarkı', u, 80) },
        { title: 'see https://example.com', url: u, max: 80, output: link('see https://example.com', u, 80) },
        { title: '🔥🔥', url: u, max: 80, output: link('🔥🔥', u, 80) },
        { title: null, url: u, max: 80, output: link(null, u, 80) },
    ],
    formatUptime: [
        { ms: 0, output: formatUptime(0) },
        { ms: 1000, output: formatUptime(1000) },
        { ms: 60_000, output: formatUptime(60_000) },
        { ms: 61 * 60_000, output: formatUptime(61 * 60_000) },
        { ms: 1500 * 60_000, output: formatUptime(1500 * 60_000) },
        { ms: 86400000 + 3600000 + 120000, output: formatUptime(86400000 + 3600000 + 120000) },
    ],
    chunk: [
        { items: [1, 2, 3, 4, 5], size: 2, output: chunk([1, 2, 3, 4, 5], 2) },
        { items: [1, 2, 3], size: 5, output: chunk([1, 2, 3], 5) },
    ],
};

const outDir = path.join(__dirname, '../rust/crates/nucleo/tests/fixtures');
fs.mkdirSync(outDir, { recursive: true });
fs.writeFileSync(path.join(outDir, 'formato-node.json'), JSON.stringify(fixture, null, 2), 'utf-8');
console.log('Generated formato-node.json successfully');
