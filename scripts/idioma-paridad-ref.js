'use strict';

const fs = require('node:fs');
const path = require('node:path');
const i18n = require('../src/core/i18n');

const targetPath = path.join(
    __dirname,
    '..',
    'rust',
    'crates',
    'nucleo',
    'tests',
    'data',
    't-casos.json',
);

const cleanCases = [
    '✅ Done 🎵 now',
    '  Hello   world \t test  ',
    '  Line 1\n   Line 2\n\tLine 3  ',
    '🎵 Music 🎶 Note 🎧 Headphone',
    'Flag: 🇺🇸 US and skin tone: 👍🏽 thumbs up',
    'Keycap 1️⃣ one',
    'Normal text without emoji',
    '',
    '   leading and trailing spaces   ',
    'Multiple \n\n   lines \n   with spaces  ',
];

const tCases = [
    { lang: 'tr', key: 'buttonhandler.songs_shuffled', vars: { count: '3' } },
    { lang: 'xx', key: 'buttons.skip', vars: {} },
    { lang: 'en', key: 'does.not.exist', vars: {} },
    { lang: 'en', key: 'commands.help.stats_servers', vars: { count: '42' } },
    { lang: 'en', key: 'commands.help.stats_servers', vars: {} },
    { lang: 'es', key: 'commands.language.changed_desc', vars: { language: 'Español' } },
    { lang: 'es', key: 'commands.nowplaying.volume', vars: { volume: '80' } },
    { lang: 'ja', key: 'commands.search.no_results', vars: {} },
    { lang: 'zh_CN', key: 'commands.play.voice_channel_required', vars: {} },
    { lang: 'zh_TW', key: 'commands.play.voice_channel_required', vars: {} },
    { lang: 'de', key: 'commands.play.track_started', vars: { title: 'Song 🎵 Title' } },
    { lang: 'fr', key: 'commands.play.track_queued', vars: { title: 'Title', position: '5' } },
    { lang: 'pt', key: 'commands.language.title', vars: {} },
    { lang: 'ru', key: 'commands.language.title', vars: {} },
];

const discordLocales = [
    'en-US', 'en-GB', 'tr', 'de', 'es-ES', 'es-419', 'fr', 'it',
    'nl', 'pl', 'pt-BR', 'ru', 'sv-SE', 'no', 'da', 'fi',
    'cs', 'ja', 'ko', 'zh-CN', 'zh-TW', 'hi', 'th', 'id',
    'xx',
];

const out = {
    clean: cleanCases.map((input) => ({ input, output: i18n.clean(input) })),
    t: tCases.map(({ lang, key, vars }) => ({
        lang,
        key,
        vars,
        output: i18n.t(lang, key, vars),
    })),
    discordLocales: discordLocales.map((locale) => ({
        locale,
        output: i18n.fromDiscordLocale(locale),
    })),
    list: i18n.list(),
    localizationsPlay: i18n.localizations('commands.play.description'),
    localizationsLangTitle: i18n.localizations('commands.language.title'),
};

fs.mkdirSync(path.dirname(targetPath), { recursive: true });
fs.writeFileSync(targetPath, JSON.stringify(out, null, 2) + '\n');
console.log(`Generated i18n reference cases: ${targetPath}`);
