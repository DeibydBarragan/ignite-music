'use strict';

/**
 * MusicManager facade (Phase 2.1): owns DisTube instance, per-guild state and event wiring.
 * Delegates to playback.js, panel.js, presence.js and sessions-restore.js.
 */

const { DisTube } = require('distube');
const { SpotifyPlugin } = require('@distube/spotify');
const config = require('../config');
const i18n = require('../core/i18n');
const { YouTubePlugin, GenericPlugin, RelayedDirectLinkPlugin } = require('./plugins');
const { getRelay } = require('./relay');
const { SessionStore } = require('./session');
const { ApiOps } = require('./api-ops');
const { describeError, hostHint } = require('./errors');
const panel = require('./panel');
const playback = require('./playback');
const presence = require('./presence');
const sessionsRestore = require('./sessions-restore');
const { bindDisTubeEvents, resolveFfmpeg } = require('./events');
const log = require('../core/logger').createLogger('music');

const SESSION_SAVE_INTERVAL_MS = 15_000;

class MusicManager {
    constructor(client, deps = {}) {
        this.client = client;
        this.shuttingDown = false;
        this.guilds = new Map();

        this.relay = deps.relay || getRelay();
        this.youtube = deps.youtube || new YouTubePlugin({ relay: this.relay });
        this.spotify = deps.spotify || new SpotifyPlugin(config.spotify.clientId && config.spotify.clientSecret
            ? { api: { clientId: config.spotify.clientId, clientSecret: config.spotify.clientSecret } }
            : {});
        this.direct = deps.direct || new RelayedDirectLinkPlugin({ relay: this.relay });
        this.generic = deps.generic || new GenericPlugin({ relay: this.relay });

        const plugins = [this.youtube, this.spotify, this.direct, this.generic];

        this.distube = deps.distube || new DisTube(client, {
            plugins,
            savePreviousSongs: true,
            emitAddSongWhenCreatingQueue: false,
            emitAddListWhenCreatingQueue: false,
            joinNewVoiceChannel: false,
            ffmpeg: {
                path: resolveFfmpeg(),
                args: { input: { reconnect: 0, reconnect_streamed: 0, reconnect_delay_max: null } },
            },
        });

        this.sessions = deps.sessions || new SessionStore({
            youtube: this.youtube,
            spotify: this.spotify,
            direct: this.direct,
            generic: this.generic,
        });

        this.apiOps = new ApiOps(this);
        this.sse = deps.sse || null;

        bindDisTubeEvents(this);
        this.saveTimer = setInterval(() => {
            this.saveAllSessions().catch((error) => log.warn('Could not save sessions:', error.message));
        }, SESSION_SAVE_INTERVAL_MS);
        this.saveTimer.unref();
    }

    // ─── Per guild runtime state ───────────────────────────────────────

    state(guildId) {
        let state = this.guilds.get(guildId);
        if (!state) {
            state = { panel: null, chain: Promise.resolve(), playLock: Promise.resolve(), debounce: null, refresh: null, leave: null, empty: null, pausedForEmpty: false, disconnectReason: null };
            this.guilds.set(guildId, state);
        }
        return state;
    }

    translator(guildId) {
        return i18n.forGuild(this.client.guilds.cache.get(guildId));
    }

    getQueue(guildId) {
        return this.distube.getQueue(guildId);
    }

    snapshot(queue) {
        return panel.snapshot(queue);
    }

    panelPayload(queue) {
        return panel.panelPayload(this, queue);
    }

    // ─── Errors ────────────────────────────────────────────────────────

    /**
     * Translation key for an error shown to users; logs setup instructions for the bot owner when needed.
     */
    explainError(error, since = 0) {
        const key = describeError(error, { youtube: this.youtube, since });
        const hint = hostHint(key);
        if (hint) log.warn(hint);
        return key;
    }

    // ─── Playing (playback.js) ─────────────────────────────────────────

    play(options) {
        return playback.play(this, options);
    }

    prefetchNext(queue) {
        return playback.prefetchNext(this, queue);
    }

    /**
     * Serialize mutations of a guild's queue (play, remove, reorder...) so
     * concurrent API requests never corrupt the DisTube songs array.
     */
    withPlayLock(guildId, task) {
        const state = this.state(guildId);
        const run = state.playLock.then(task);
        state.playLock = run.catch(() => undefined);
        return run;
    }

    /**
     * After the queue was edited outside of DisTube events (remove, move, clear, shuffle...).
     */
    changed(queue, { refresh = true } = {}) {
        if (refresh) this.refreshPanel(queue.id);
        this.prefetchNext(queue);
        if (!this.shuttingDown) this.sessions.save(queue).catch((error) => log.warn(`Could not save session ${queue.id}: ${error.message}`));
        this.publishSync(queue.id);
    }

    /**
     * Push the player snapshot to SSE subscribers of that guild. Lazy requires
     * keep the API layer out of the module-load graph and make failures
     * non-fatal: streaming is best-effort, the REST state remains canonical.
     */
    publishSync(guildId) {
        if (this.shuttingDown) return;
        try {
            const hub = this.sse || require('../api/sse').getSseHub();
            if (hub.size(guildId) === 0) return;
            const { state } = require('../api/v1/ops/player');
            hub.publish(guildId, 'player.sync', state(this, { guildId }));
        } catch (error) {
            log.debug(`SSE publish failed for ${guildId}: ${error.message}`);
        }
    }

    // ─── Panel lifecycle (panel.js) ────────────────────────────────────
    sendPanel(queue) { return panel.sendPanel(this, queue); }
    refreshPanel(guildId, options) { return panel.refreshPanel(this, guildId, options); }
    endPanel(guildId, options) { return panel.endPanel(this, guildId, options); }
    replacePanel(guildId, message) { return panel.replacePanel(this, guildId, message); }
    isCurrentPanel(guildId, messageId) { return panel.isCurrentPanel(this, guildId, messageId); }
    adoptPanel(guildId, message) { return panel.adoptPanel(this, guildId, message); }

    // ─── Presence (presence.js) ────────────────────────────────────────
    listenerCount(guild, channelId) { return presence.listenerCount(this, guild, channelId); }
    handleVoiceStateUpdate(oldState, newState) { return presence.handleVoiceStateUpdate(this, oldState, newState); }

    // ─── Sessions (sessions-restore.js) ────────────────────────────────
    saveAllSessions() { return sessionsRestore.saveAllSessions(this); }
    restoreSessions() { return sessionsRestore.restoreSessions(this); }

    /**
     * Save every queue and disconnect cleanly (SIGINT/SIGTERM).
     */
    async shutdown() {
        await this.saveAllSessions().catch((error) => log.warn('Could not save sessions on shutdown:', error.message));
        this.shuttingDown = true;
        clearInterval(this.saveTimer);
        await this.sessions.drain();
        for (const state of this.guilds.values()) {
            clearTimeout(state.debounce);
            clearTimeout(state.empty);
            presence.clearLeave(state);
            panel.stopRefresh(state);
        }
        for (const voice of [...this.distube.voices.collection.values()]) {
            try {
                voice.leave();
            } catch {
                // already gone
            }
        }
    }
}

module.exports = { MusicManager, resolveFfmpeg };
