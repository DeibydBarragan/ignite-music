'use strict';

/**
 * Voice presence and idle timers (Phase 2.1): leave-on-finish scheduling and
 * the pause/resume/leave behavior when the bot is left alone in a channel.
 */

const config = require('../config');

function clearLeave(state) {
    clearTimeout(state.leave);
    state.leave = null;
}

function scheduleLeave(manager, guildId) {
    const state = manager.state(guildId);
    clearLeave(state);
    if (!config.bot.leaveOnFinishMs) return;
    state.leave = setTimeout(() => {
        state.leave = null;
        if (!manager.getQueue(guildId)) {
            state.disconnectReason = 'inactivity_finished';
            manager.distube.voices.leave(guildId);
        }
    }, config.bot.leaveOnFinishMs);
    state.leave.unref?.();
}

/**
 * Humans (non-bot users) in the bot's voice channel. Uses voice states so it works
 * without the privileged GuildMembers intent.
 */
function listenerCount(manager, guild, channelId = guild.members.me?.voice?.channelId) {
    if (!channelId) return 0;
    return guild.voiceStates.cache.filter((voiceState) =>
        voiceState.channelId === channelId && voiceState.id !== manager.client.user.id && !voiceState.member?.user?.bot).size;
}

/**
 * Pause when everyone leaves, resume when someone returns, leave after LEAVE_ON_EMPTY.
 */
async function handleVoiceStateUpdate(manager, oldState, newState) {
    const guild = newState.guild || oldState.guild;
    const queue = manager.getQueue(guild.id);
    if (!queue) return;
    const state = manager.state(guild.id);
    const listeners = listenerCount(manager, guild);

    if (listeners === 0 && !state.empty && config.bot.leaveOnEmptyMs) {
        // Arm the timer before any await so concurrent voice updates can't start a second one.
        state.empty = setTimeout(async () => {
            state.empty = null;
            const current = manager.getQueue(guild.id);
            if (!current || listenerCount(manager, guild) > 0) return;
            state.pausedForEmpty = false;
            await manager.endPanel(guild.id, { titleKey: 'musicmanager.playback_ended', descriptionKey: 'ui.left_empty' });
            await current.stop().catch(() => undefined);
            state.disconnectReason = 'inactivity_empty';
            manager.distube.voices.leave(guild.id);
        }, config.bot.leaveOnEmptyMs);
        state.empty.unref?.();
        if (!queue.paused) {
            state.pausedForEmpty = true;
            await queue.pause().catch(() => undefined);
            manager.refreshPanel(guild.id);
        }
    } else if (listeners > 0 && state.empty) {
        clearTimeout(state.empty);
        state.empty = null;
        if (state.pausedForEmpty && queue.paused) {
            await queue.resume().catch(() => undefined);
            manager.refreshPanel(guild.id);
        }
        state.pausedForEmpty = false;
    }
}

module.exports = { clearLeave, scheduleLeave, listenerCount, handleVoiceStateUpdate };
