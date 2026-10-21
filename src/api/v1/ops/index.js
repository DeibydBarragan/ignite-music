'use strict';

/**
 * Registers every REST API operation on a MusicManager's apiOps registry.
 * Called once per shard (bot.js) right after the manager is created, so any
 * shard can serve operations for the guilds it owns.
 */

const playerOps = require('./player');
const queueOps = require('./queue');
const discoveryOps = require('./discovery');
const voiceOps = require('./voice');

function registerApiOps(music) {
    if (!music?.apiOps) throw new Error('registerApiOps requires a MusicManager with an apiOps registry');
    playerOps.register(music.apiOps);
    queueOps.register(music.apiOps);
    discoveryOps.register(music.apiOps);
    voiceOps.register(music.apiOps);
    return music.apiOps;
}

module.exports = { registerApiOps };
