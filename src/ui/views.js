'use strict';

const {
    ActionRowBuilder,
    ButtonBuilder,
    ButtonStyle,
    ContainerBuilder,
    MessageFlags,
    SectionBuilder,
    SeparatorBuilder,
    SeparatorSpacingSize,
    StringSelectMenuBuilder,
    TextDisplayBuilder,
    ThumbnailBuilder,
} = require('discord.js');
const config = require('../config');
const { formatDuration, progressBar, truncate, link, escapeMarkdown, safeUrl } = require('../core/format');
const { resolveRequester } = require('../core/user');
const { FILTERS, FILTER_LABELS } = require('../music/filters');

const V2 = MessageFlags.IsComponentsV2;
const EPHEMERAL_V2 = MessageFlags.IsComponentsV2 | MessageFlags.Ephemeral;

const ID = {
    previous: 'm:prev',
    pause: 'm:pause',
    skip: 'm:skip',
    stop: 'm:stop',
    volume: 'm:vol',
    shuffle: 'm:shuffle',
    loop: 'm:loop',
    autoplay: 'm:auto',
    queue: 'm:queue',
    lyrics: 'm:lyrics',
    filter: 'm:filter',
    favorite: 'm:fav',
    volumeModal: 'm:volmodal',
    volumeInput: 'volume',
    queuePage: 'q:',
    lyricsPage: 'ly:',
    search: 's:',
    language: 'lang:set',
};

const text = (content) => new TextDisplayBuilder().setContent(content);
const separator = (large = false) => new SeparatorBuilder()
    .setDivider(true)
    .setSpacing(large ? SeparatorSpacingSize.Large : SeparatorSpacingSize.Small);
const container = (color = config.bot.color) => new ContainerBuilder().setAccentColor(color);
const button = (customId, label, style = ButtonStyle.Secondary) => new ButtonBuilder()
    .setCustomId(customId)
    .setLabel(truncate(label, 80))
    .setStyle(style);
const row = (...components) => new ActionRowBuilder().addComponents(...components);

/**
 * Components v2 reply shape shared by every view: flags + no mention parsing.
 */
function baseView(components, { ephemeral = false } = {}) {
    return { components, flags: ephemeral ? EPHEMERAL_V2 : V2, allowedMentions: { parse: [] } };
}

/**
 * Section with a thumbnail when a valid image URL exists, plain text otherwise.
 */
function headline(c, content, thumbnail) {
    const image = safeUrl(thumbnail);
    if (image) {
        c.addSectionComponents(new SectionBuilder()
            .addTextDisplayComponents(text(content))
            .setThumbnailAccessory(new ThumbnailBuilder().setURL(image)));
    } else {
        c.addTextDisplayComponents(text(content));
    }
}

/**
 * A short status message (ephemeral by default).
 */
function notice(message, { ephemeral = true, color } = {}) {
    return baseView([container(color).addTextDisplayComponents(text(truncate(message, 3900)))], { ephemeral });
}

const LOOP_KEYS = ['buttons.loop_off', 'buttons.loop_track', 'buttons.loop_queue'];

function loopLabel(t, mode) {
    return t(LOOP_KEYS[mode] || LOOP_KEYS[0]);
}

function filterLabel(t, name) {
    return name ? FILTER_LABELS[name] || name : t('ui.filter_none');
}

const requesterUserOf = resolveRequester;

function requesterLine(t, song) {
    const user = requesterUserOf(song);
    if (!user || user.bot) return t('ui.autoplay_added');
    return `${t('commands.nowplaying.requested_by')}: <@${user.id}>`;
}

/**
 * The now-playing panel. `state` is a plain object so it can be tested without Discord.
 */
function playerPanel(t, state) {
    const { song, paused, currentTime = 0, volume, repeatMode = 0, autoplay = false, filter = null, upcoming = [], upcomingDuration = 0 } = state;
    const c = container();

    const status = paused ? t('commands.nowplaying.status_paused') : t('commands.nowplaying.title');
    const meta = [song.uploader?.name ? escapeMarkdown(truncate(song.uploader.name, 60)) : null,
        song.isLive ? t('ui.live') : formatDuration(song.duration)].filter(Boolean).join(' · ');
    headline(c, `-# ${status}\n### ${link(song.name, song.url, 90)}\n${meta}`, song.thumbnail);

    if (!song.isLive && song.duration > 0) {
        const position = Math.min(currentTime, song.duration);
        c.addTextDisplayComponents(text(`\`${formatDuration(position)}\` ${progressBar(position, song.duration)} \`${formatDuration(song.duration)}\``));
    }

    const details = [
        requesterLine(t, song),
        t('commands.nowplaying.volume', { volume }),
        loopLabel(t, repeatMode),
        autoplay ? t('buttons.autoplay_on') : t('buttons.autoplay_off'),
        filter ? t('ui.filter', { filter: filterLabel(t, filter) }) : null,
    ].filter(Boolean);
    c.addTextDisplayComponents(text(`-# ${details.join(' · ')}`));

    const next = upcoming[0];
    const nextUser = requesterUserOf(next);
    const nextReq = nextUser
        ? (nextUser.bot ? ` · ${t('ui.autoplay_added')}` : ` · <@${nextUser.id}>`)
        : '';
    const queueLine = next
        ? `${t('commands.nowplaying.next_song')}: ${link(next.name, next.url, 60)}${nextReq}\n-# ${t('commands.nowplaying.footer_more_songs', { count: upcoming.length })} · ${formatDuration(upcomingDuration)}`
        : `-# ${t('commands.nowplaying.footer_no_songs')}`;
    c.addTextDisplayComponents(text(queueLine));

    c.addSeparatorComponents(separator());
    c.addActionRowComponents(
        row(
            button(ID.previous, t('ui.previous')),
            button(ID.pause, paused ? t('buttons.resume') : t('buttons.pause'), ButtonStyle.Primary),
            button(ID.skip, t('buttons.skip')),
            button(ID.stop, t('buttons.stop'), ButtonStyle.Danger),
            button(ID.volume, t('buttons.volume')),
        ),
        row(
            button(ID.shuffle, t('buttons.shuffle')).setDisabled(upcoming.length < 2),
            button(ID.loop, loopLabel(t, repeatMode), repeatMode ? ButtonStyle.Success : ButtonStyle.Secondary),
            button(ID.autoplay, autoplay ? t('buttons.autoplay_on') : t('buttons.autoplay_off'), autoplay ? ButtonStyle.Success : ButtonStyle.Secondary),
            button(ID.queue, t('buttons.queue')),
            button(ID.favorite, t('buttons.favorite')),
        ),
        row(new StringSelectMenuBuilder()
            .setCustomId(ID.filter)
            .setPlaceholder(`${t('ui.filter_placeholder')}: ${filterLabel(t, filter)}`)
            .addOptions(
                { label: t('ui.filter_none'), value: 'none', default: !filter },
                ...FILTERS.map((name) => ({ label: FILTER_LABELS[name], value: name, default: filter === name })),
            )),
        row(button(ID.lyrics, t('buttons.lyrics'))),
    );

    return baseView([c]);
}

/**
 * Panel after playback ended (queue finished, stopped, disconnected...). No controls.
 */
function endedPanel(t, { title, description, song } = {}) {
    const c = container();
    const lines = [`### ${title}`];
    if (description) lines.push(description);
    if (song) lines.push(`-# ${link(song.name, song.url, 90)}`);
    c.addTextDisplayComponents(text(lines.join('\n')));
    return baseView([c]);
}

const { createAuxiliaryViews } = require('./auxiliary-views');
const auxiliary = createAuxiliaryViews({ ID, baseView, container, text, separator, button, row });

module.exports = {
    ID,
    FILTERS,
    FILTER_LABELS,
    V2,
    EPHEMERAL_V2,
    QUEUE_PAGE_SIZE: auxiliary.QUEUE_PAGE_SIZE,
    PLAYLISTS_PAGE_SIZE: auxiliary.PLAYLISTS_PAGE_SIZE,
    baseView,
    notice,
    playerPanel,
    endedPanel,
    queueView: auxiliary.queueView,
    lyricsView: auxiliary.lyricsView,
    searchView: auxiliary.searchView,
    helpView: auxiliary.helpView,
    languageView: auxiliary.languageView,
    playlistsView: auxiliary.playlistsView,
    splitLyrics: auxiliary.splitLyrics,
    loopLabel,
    filterLabel,
};
