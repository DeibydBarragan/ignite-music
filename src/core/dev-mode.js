'use strict';

const fs = require('node:fs');
const path = require('node:path');
const config = require('../config');
const logger = require('./logger');

const log = logger.createLogger('dev-mode');
const DEV_FILE = path.join(config.dataDir, 'dev_users.json');

const activeDevUsers = new Set();

function loadDevUsers() {
    try {
        if (!fs.existsSync(DEV_FILE)) return;
        const raw = fs.readFileSync(DEV_FILE, 'utf8');
        const parsed = JSON.parse(raw);
        if (Array.isArray(parsed?.users)) {
            for (const uid of parsed.users) {
                if (typeof uid === 'string' && uid.trim()) {
                    activeDevUsers.add(uid.trim());
                }
            }
        }
    } catch (err) {
        log.warn(`Failed to load dev users file: ${err.message}`);
    }
}

function persistDevUsers() {
    try {
        const dir = path.dirname(DEV_FILE);
        if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
        const data = {
            users: Array.from(activeDevUsers),
            updatedAt: new Date().toISOString(),
        };
        fs.writeFileSync(DEV_FILE, JSON.stringify(data, null, 2), 'utf8');
    } catch (err) {
        log.warn(`Failed to save dev users file: ${err.message}`);
    }
}

// Initial load
loadDevUsers();

/**
 * Checks whether a user ID currently has developer mode enabled.
 * @param {string} userId - Discord snowflake user ID.
 * @returns {boolean}
 */
function isDevMode(userId) {
    if (!userId) return false;
    return activeDevUsers.has(String(userId).trim());
}

/**
 * Enables or disables developer mode for a given user ID.
 * @param {string} userId - Discord snowflake user ID.
 * @param {boolean} enabled
 * @returns {boolean} Updated state.
 */
function setDevMode(userId, enabled) {
    if (!userId) return false;
    const uid = String(userId).trim();
    if (enabled) {
        activeDevUsers.add(uid);
    } else {
        activeDevUsers.delete(uid);
    }
    persistDevUsers();
    log.info(`Dev mode for user ${uid} set to ${Boolean(enabled)}`);
    return activeDevUsers.has(uid);
}

/**
 * Returns all user IDs currently in dev mode.
 * @returns {string[]}
 */
function getDevUsers() {
    return Array.from(activeDevUsers);
}

module.exports = {
    isDevMode,
    setDevMode,
    getDevUsers,
};
