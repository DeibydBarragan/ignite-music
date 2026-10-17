'use strict';

/**
 * Player panel lifecycle (Phase 2.1): sending, refreshing, ending, replacing
 * and adopting the Discord panel. Every function takes the MusicManager
 * facade as its first argument; per-guild state lives in manager.state().
 */

const { RESTJSONErrorCodes } = require('discord.js');
const config = require('../config');
const views = require('../ui/views');
const log = require('../core/logger').createLogger('panel');

const PANEL_DEBOUNCE_MS = 750;

const DISCORD_GONE = new Set([
    RESTJSONErrorCodes.UnknownMessage,
    RESTJSONErrorCodes.UnknownChannel,
    RESTJSONErrorCodes.MissingAccess,
    RESTJSONErrorCodes.MissingPermissions,
]);

/**
 * Run panel operations for a guild one after another so sends/edits never race.
 */
function enqueue(manager, guildId, task) {
    const state = manager.state(guildId);
    state.chain = state.chain.then(task).catch((error) => log.warn(`Panel update failed in ${guildId}: ${error.message}`));
    return state.chain;
}

function forgetPanelOnError(state, error) {
    if (DISCORD_GONE.has(error?.code)) {
        state.panel = null;
        return true;
    }
    return false;
}

/**
 * Send a fresh panel for a new song. Re-uses the old panel when it is still the latest
 * message in the channel, otherwise deletes it and posts a new one at the bottom.
 */
function sendPanel(manager, queue) {
    return enqueue(manager, queue.id, async () => {
        const state = manager.state(queue.id);
        const channel = queue.textChannel;
        if (!channel || queue.stopped || !queue.songs.length) return;
        const payload = manager.panelPayload(queue);

        if (state.panel && state.panel.channelId === channel.id && channel.lastMessageId === state.panel.id) {
            try {
                await state.panel.edit(payload);
                return;
            } catch (error) {
                if (!forgetPanelOnError(state, error)) throw error;
            }
        }
        if (state.panel) {
            const old = state.panel;
            state.panel = null;
            old.delete().catch(() => undefined);
        }
        state.panel = await channel.send(payload);
    });
}

/**
 * Edit the current panel (debounced) after a state change.
 */
function refreshPanel(manager, guildId, { immediate = false } = {}) {
    const state = manager.state(guildId);
    clearTimeout(state.debounce);
    const run = () => enqueue(manager, guildId, async () => {
        const queue = manager.getQueue(guildId);
        if (!state.panel || !queue || queue.stopped || !queue.songs.length) return;
        try {
            await state.panel.edit(manager.panelPayload(queue));
        } catch (error) {
            if (!forgetPanelOnError(state, error)) throw error;
        }
    });
    if (immediate) return run();
    state.debounce = setTimeout(run, PANEL_DEBOUNCE_MS);
    state.debounce.unref?.();
    return undefined;
}

/**
 * Replace the panel with a final, control-less message.
 */
function endPanel(manager, guildId, { titleKey, descriptionKey, song } = {}) {
    const state = manager.state(guildId);
    clearTimeout(state.debounce);
    stopRefresh(state);
    return enqueue(manager, guildId, async () => {
        if (!state.panel) return;
        const t = manager.translator(guildId);
        const panel = state.panel;
        state.panel = null;
        await panel.edit(views.endedPanel(t, {
            title: t(titleKey),
            description: descriptionKey ? t(descriptionKey) : null,
            song,
        })).catch(() => undefined);
    });
}

/**
 * Make `message` the panel (e.g. /nowplaying reply) and delete the previous one.
 */
function replacePanel(manager, guildId, message) {
    return enqueue(manager, guildId, async () => {
        const state = manager.state(guildId);
        clearTimeout(state.debounce);
        const old = state.panel;
        state.panel = message;
        if (old && old.id !== message.id) await old.delete().catch(() => undefined);
    });
}

function isCurrentPanel(manager, guildId, messageId) {
    return manager.state(guildId).panel?.id === messageId;
}

/**
 * Called after a button already updated the panel through the interaction.
 */
function adoptPanel(manager, guildId, message) {
    const state = manager.state(guildId);
    clearTimeout(state.debounce);
    if (message) state.panel = message;
}

function startRefresh(manager, queue) {
    const state = manager.state(queue.id);
    stopRefresh(state);
    if (!config.bot.panelRefreshMs) return;
    state.refresh = setInterval(() => {
        const current = manager.getQueue(queue.id);
        if (!current || current.stopped) return stopRefresh(state);
        if (!current.paused) manager.refreshPanel(queue.id, { immediate: true });
    }, config.bot.panelRefreshMs);
    state.refresh.unref?.();
}

function stopRefresh(state) {
    clearInterval(state.refresh);
    state.refresh = null;
}

function snapshot(queue) {
    const upcoming = queue.songs.slice(1);
    return {
        song: queue.songs[0],
        paused: queue.paused,
        currentTime: queue.currentTime,
        volume: queue.volume,
        repeatMode: queue.repeatMode,
        autoplay: queue.autoplay,
        filter: queue.filters.names[0] || null,
        upcoming,
        upcomingDuration: upcoming.reduce((sum, song) => sum + (song.duration || 0), 0),
    };
}

function panelPayload(manager, queue) {
    return views.playerPanel(manager.translator(queue.id), snapshot(queue));
}

module.exports = {
    PANEL_DEBOUNCE_MS,
    sendPanel,
    refreshPanel,
    endPanel,
    replacePanel,
    isCurrentPanel,
    adoptPanel,
    startRefresh,
    stopRefresh,
    snapshot,
    panelPayload,
};
