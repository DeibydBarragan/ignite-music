'use strict';

/**
 * Developer Mode routes:
 * Allows authorized users (or developers) to toggle developer mode for their Discord user ID.
 * When enabled, song playbacks initiated by this user are omitted from Supabase 'song_plays' database storage.
 */

const { isDevMode, setDevMode } = require('../../core/dev-mode');

function register(router) {
    router.get('/user/dev-mode', (req, res) => {
        const rawUserId = req.discordUserId || req.headers['x-user-discord-id'] || req.query.userId;
        const userId = typeof rawUserId === 'string' ? rawUserId.trim() : null;
        return res.json({
            success: true,
            devMode: isDevMode(userId),
            userId: userId || null,
        });
    });

    router.post('/user/dev-mode', (req, res) => {
        const rawUserId = req.discordUserId || req.headers['x-user-discord-id'] || req.body?.userId;
        const userId = typeof rawUserId === 'string' ? rawUserId.trim() : null;

        if (!userId) {
            return res.status(400).json({
                success: false,
                error: 'MISSING_USER_ID',
                detail: 'X-User-Discord-Id header or userId is required',
            });
        }

        const enabled = Boolean(req.body?.enabled);
        const updated = setDevMode(userId, enabled);

        return res.json({
            success: true,
            devMode: updated,
            userId,
        });
    });
}

module.exports = { register };
