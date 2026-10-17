'use strict';

const { ButtonBuilder, ButtonStyle, StringSelectMenuBuilder } = require('discord.js');
const config = require('../config');
const { formatDuration, truncate, link, escapeMarkdown, safeUrl } = require('../core/format');
const { resolveRequester } = require('../core/user');

const QUEUE_PAGE_SIZE = 10;
const LYRICS_PAGE_SIZE = 2800;
const PLAYLISTS_PAGE_SIZE = 10;

function createAuxiliaryViews({ ID, baseView, container, text, separator, button, row }) {
    const requesterUserOf = resolveRequester;

    function pager(prefix, page, pages, t) {
        return row(
            button(`${prefix}${page - 1}`, t('ui.back')).setDisabled(page <= 0),
            button(`${prefix}${page + 1}`, t('ui.next')).setDisabled(page >= pages - 1),
        );
    }

    function queueView(t, { current, upcoming, page = 0 }) {
        const pages = Math.max(1, Math.ceil(upcoming.length / QUEUE_PAGE_SIZE));
        const safePage = Math.min(Math.max(0, page), pages - 1);
        const total = upcoming.reduce((sum, song) => sum + (song.duration || 0), current?.duration || 0);
        const c = container();

        const header = [`### ${t('buttons.queue')}`];
        if (current) {
            const u = requesterUserOf(current);
            const reqStr = u
                ? (u.bot ? ` · ${t('ui.autoplay_added')}` : ` · <@${u.id}>`)
                : '';
            header.push(`${t('buttonhandler.now_playing')}: ${link(current.name, current.url, 70)} · ${current.isLive ? t('ui.live') : formatDuration(current.duration)}${reqStr}`);
        }
        c.addTextDisplayComponents(text(header.join('\n')));

        if (upcoming.length) {
            const start = safePage * QUEUE_PAGE_SIZE;
            const lines = upcoming.slice(start, start + QUEUE_PAGE_SIZE).map((song, index) => {
                const u = requesterUserOf(song);
                const req = u
                    ? (u.bot ? ` · *${t('ui.autoplay_added')}*` : ` · <@${u.id}>`)
                    : '';
                return `\`${String(start + index + 1).padStart(2, ' ')}.\` ${link(song.name, song.url, 50)} · ${song.isLive ? t('ui.live') : formatDuration(song.duration)}${req}`;
            });
            c.addSeparatorComponents(separator());
            c.addTextDisplayComponents(text(`**${t('buttonhandler.upcoming_songs', { count: upcoming.length })}**\n${lines.join('\n')}`));
        } else {
            c.addTextDisplayComponents(text(`-# ${t('commands.nowplaying.footer_no_songs')}`));
        }

        c.addTextDisplayComponents(text(`-# ${t('buttonhandler.total_songs', { count: upcoming.length + (current ? 1 : 0) })} · ${formatDuration(total)} · ${t('ui.page', { page: safePage + 1, pages })}`));
        if (pages > 1) c.addActionRowComponents(pager(ID.queuePage, safePage, pages, t));
        return baseView([c], { ephemeral: true });
    }

    function splitLyrics(lyrics) {
        const pages = [];
        let current = '';
        for (const paragraph of String(lyrics).split(/\n{2,}/)) {
            const block = paragraph.trim();
            if (!block) continue;
            if (current && current.length + block.length + 2 > LYRICS_PAGE_SIZE) {
                pages.push(current);
                current = '';
            }
            if (block.length > LYRICS_PAGE_SIZE) {
                for (let i = 0; i < block.length; i += LYRICS_PAGE_SIZE) pages.push(block.slice(i, i + LYRICS_PAGE_SIZE));
                continue;
            }
            current = current ? `${current}\n\n${block}` : block;
        }
        if (current) pages.push(current);
        return pages.length ? pages : [''];
    }

    function lyricsView(t, { title, artist, lyrics, source, token, page = 0 }) {
        const pages = splitLyrics(lyrics);
        const safePage = Math.min(Math.max(0, page), pages.length - 1);
        const c = container();
        const subtitle = [escapeMarkdown(truncate(title, 80)), artist ? escapeMarkdown(truncate(artist, 60)) : null].filter(Boolean).join(' · ');
        c.addTextDisplayComponents(text(`### ${t('buttonhandler.lyrics_title')}\n-# ${subtitle}`));
        c.addSeparatorComponents(separator());
        c.addTextDisplayComponents(text(escapeMarkdown(pages[safePage]).replace(/^([#-])/gm, '\\$1')));
        const footer = [source ? `${source}` : null, pages.length > 1 ? t('ui.page', { page: safePage + 1, pages: pages.length }) : null].filter(Boolean);
        if (footer.length) c.addTextDisplayComponents(text(`-# ${footer.join(' · ')}`));
        if (pages.length > 1 && token) c.addActionRowComponents(pager(`${ID.lyricsPage}${token}:`, safePage, pages.length, t));
        return baseView([c], { ephemeral: true });
    }

    function searchView(t, { query, results, token }) {
        const c = container();
        const lines = results.map((song, index) =>
            `\`${index + 1}.\` ${link(song.name, song.url, 70)}\n-# ${[song.uploader?.name ? escapeMarkdown(truncate(song.uploader.name, 40)) : null, song.isLive ? t('ui.live') : formatDuration(song.duration)].filter(Boolean).join(' · ')}`);
        c.addTextDisplayComponents(text(`### ${t('commands.search.title', { query: escapeMarkdown(truncate(query, 80)) })}\n${lines.join('\n')}`));
        c.addSeparatorComponents(separator());
        c.addActionRowComponents(row(new StringSelectMenuBuilder()
            .setCustomId(`${ID.search}${token}`)
            .setPlaceholder(truncate(t('commands.search.select_description'), 150))
            .addOptions(results.map((song, index) => ({
                label: truncate(`${index + 1}. ${song.name}`, 100),
                description: truncate([song.uploader?.name, song.isLive ? t('ui.live') : formatDuration(song.duration)].filter(Boolean).join(' · '), 100),
                value: String(index),
            })))));
        return baseView([c], { ephemeral: true });
    }

    function playlistsTotalDuration(pl) {
        return (pl.tracks || []).reduce((sum, track) => sum + (track.duration || 0), 0);
    }

    function playlistsView(t, { playlists, total, page = 0, filter = null }) {
        const pages = Math.max(1, Math.ceil(playlists.length / PLAYLISTS_PAGE_SIZE));
        const safePage = Math.min(Math.max(0, page), pages - 1);
        const c = container();
        c.addTextDisplayComponents(text(`### ${filter
            ? t('ui.playlists_title_query', { filter: truncate(filter, 40), count: total })
            : t('ui.playlists_title', { count: total })}`));

        if (!playlists.length) {
            c.addTextDisplayComponents(text(`-# ${t('ui.playlists_empty')}`));
        } else {
            const start = safePage * PLAYLISTS_PAGE_SIZE;
            const lines = playlists.slice(start, start + PLAYLISTS_PAGE_SIZE).map((pl, index) => {
                const count = (pl.tracks || []).length;
                const duration = formatDuration(playlistsTotalDuration(pl));
                return `\`${String(start + index + 1).padStart(2, ' ')}.\` **${escapeMarkdown(truncate(pl.name, 60))}**\n-# ${t('ui.playlists_line', { source: pl.source, count, duration })}`;
            });
            c.addSeparatorComponents(separator());
            c.addTextDisplayComponents(text(lines.join('\n')));
        }

        c.addTextDisplayComponents(text(`-# ${t('ui.page', { page: safePage + 1, pages })}\n-# ${t('ui.playlists_hint')}`));
        return baseView([c], { ephemeral: true });
    }

    function helpView(t, { commands, stats }) {
        const c = container();
        c.addTextDisplayComponents(text(`### ${t('commands.help.title')}\n${t('commands.help.main_description')}`));
        c.addSeparatorComponents(separator());
        const lines = commands.map((command) => `</${command.name}:${command.id || '0'}> ${command.description}`);
        const plain = commands.map((command) => `\`/${command.name}\` ${command.description}`);
        c.addTextDisplayComponents(text(`**${t('commands.help.commands_title')}**\n${(commands.every((command) => command.id) ? lines : plain).join('\n')}`));
        if (stats) {
            c.addSeparatorComponents(separator());
            c.addTextDisplayComponents(text(`-# ${[
                t('commands.help.stats_servers', { count: stats.servers }),
                t('commands.help.stats_active', { count: stats.active }),
                t('commands.help.stats_uptime', { time: stats.uptime }),
                `${stats.ping} ms`,
            ].join(' · ')}`));
        }
        const links = [
            config.bot.supportServer && [t('commands.help.button_support'), config.bot.supportServer],
            config.bot.website && [t('commands.help.button_website'), config.bot.website],
            config.bot.invite && [t('ui.invite'), config.bot.invite],
        ].filter((item) => item && safeUrl(item[1]));
        if (links.length) {
            c.addActionRowComponents(row(...links.map(([label, url]) => new ButtonBuilder().setStyle(ButtonStyle.Link).setLabel(label).setURL(url))));
        }
        return baseView([c], { ephemeral: true });
    }

    function languageView(t, { languages, current }) {
        const c = container();
        const currentName = languages.find((lang) => lang.code === current)?.name || current;
        c.addTextDisplayComponents(text(`### ${t('commands.language.title')}\n${t('commands.language.current')}: **${currentName}**`));
        c.addActionRowComponents(row(new StringSelectMenuBuilder()
            .setCustomId(ID.language)
            .setPlaceholder(truncate(t('commands.language.select'), 150))
            .addOptions(languages.slice(0, 25).map((lang) => ({ label: lang.name, value: lang.code, default: lang.code === current })))));
        return baseView([c], { ephemeral: true });
    }

    return {
        pager,
        queueView,
        splitLyrics,
        lyricsView,
        searchView,
        playlistsView,
        helpView,
        languageView,
        QUEUE_PAGE_SIZE,
        PLAYLISTS_PAGE_SIZE,
    };
}

module.exports = { createAuxiliaryViews };
