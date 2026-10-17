'use strict';

const fs = require('node:fs');
const { Events: DisTubeEvents } = require('distube');
const config = require('../config');
const views = require('../ui/views');
const history = require('./history');
const panel = require('./panel');
const presence = require('./presence');
const log = require('../core/logger').createLogger('music');

/**
 * ffmpeg: FFMPEG_PATH > ffmpeg-static (if installed) > ffmpeg on PATH.
 */
function resolveFfmpeg() {
    if (config.ffmpegPath) return config.ffmpegPath;
    try {
        const bundled = require('ffmpeg-static');
        if (bundled && fs.existsSync(bundled)) return bundled;
    } catch {
        // optional dependency not installed
    }
    return 'ffmpeg';
}

/**
 * Wire DisTube lifecycle events to panel, presence, session store and SSE.
 */
function bindDisTubeEvents(manager) {
    const d = manager.distube;

    d.on(DisTubeEvents.INIT_QUEUE, (queue) => {
        queue.volume = config.bot.defaultVolume;
        presence.clearLeave(manager.state(queue.id));
    });

    d.on(DisTubeEvents.PLAY_SONG, (queue, song) => {
        if (song?.timing) {
            const { requestedAt, resolvedAt } = song.timing;
            delete song.timing;
            const source = song.source;
            log.info(`Playing "${song.name}" from ${source}: found in ${((resolvedAt - requestedAt) / 1000).toFixed(1)}s, audio after ${((Date.now() - requestedAt) / 1000).toFixed(1)}s`);
        }
        presence.clearLeave(manager.state(queue.id));
        manager.sendPanel(queue);
        panel.startRefresh(manager, queue);
        manager.prefetchNext(queue);
        if (!manager.shuttingDown) manager.sessions.save(queue).catch((error) => log.warn(`Could not save session ${queue.id}: ${error.message}`));
        manager.publishSync(queue.id);
        history.recordPlay(queue.id, song).catch((err) => log.debug(`History record error: ${err.message}`));
    });

    const onQueueChange = (queue) => manager.changed(queue);
    d.on(DisTubeEvents.ADD_SONG, onQueueChange);
    d.on(DisTubeEvents.ADD_LIST, onQueueChange);

    d.on(DisTubeEvents.FINISH, (queue) => {
        manager.endPanel(queue.id, { titleKey: 'musicplayer.queue_completed', descriptionKey: 'musicplayer.queue_completed_desc' });
        presence.scheduleLeave(manager, queue.id);
        manager.publishSync(queue.id);
    });

    d.on(DisTubeEvents.DISCONNECT, (queue) => {
        manager.endPanel(queue.id, { titleKey: 'musicmanager.playback_ended' });
        manager.publishSync(queue.id);
    });

    d.on(DisTubeEvents.DELETE_QUEUE, (queue) => {
        const state = manager.state(queue.id);
        clearTimeout(state.empty);
        state.empty = null;
        state.pausedForEmpty = false;
        panel.stopRefresh(state);
        if (!manager.shuttingDown) manager.sessions.remove(queue.id);
        manager.publishSync(queue.id);
    });

    d.on(DisTubeEvents.NO_RELATED, (queue) => {
        const t = manager.translator(queue.id);
        queue.textChannel?.send(views.notice(t('ui.no_related'), { ephemeral: false })).catch(() => undefined);
    });

    d.on(DisTubeEvents.ERROR, (error, queue, song) => {
        // ffmpeg only reports "exited with code 1"; the relay knows what yt-dlp said.
        const played = song?.stream?.playFromSource ? song : song?.stream?.song;
        const cause = manager.relay.errorFor(played?.stream?.url);
        const detail = cause?.message || error?.message || 'Unknown playback error';
        log.error(`[${queue?.id ?? '?'}] ${song ? `${song.name}: ` : ''}${detail}`);
        if (error?.stack) log.debug(error.stack);
        if (cause?.stderr) log.warn(`[yt-dlp stderr] ${cause.stderr.trim()}`);

        if (!queue?.textChannel || manager.shuttingDown) return;
        const t = manager.translator(queue.id);
        const header = song ? `${t('musicplayer.track_could_not_play')} **${song.name}**\n` : '';
        const key = manager.explainError(cause || error, Date.now() - 60_000);
        const extra = key === 'errors.unknown' ? `\n-# Detalle: ${detail.slice(0, 200)}` : '';
        queue.textChannel.send(views.notice(`${header}${t(key)}${extra}`, { ephemeral: false })).catch(() => undefined);
    });

    if (config.debug) {
        d.on(DisTubeEvents.DEBUG, (message) => log.debug(message));
        d.on(DisTubeEvents.FFMPEG_DEBUG, (message) => log.debug(message));
    }
}

module.exports = { bindDisTubeEvents, resolveFfmpeg };
