'use strict';

/**
 * Single source of truth for "who requested this song" (Phase 2.3):
 * previously duplicated in ui/views.js, music/history.js, music/session.js
 * and api/contracts.js.
 */

/**
 * Resolve the requester user object of a DisTube Song.
 * Prefers real users (never a bot) over stored metadata; falls back to the
 * raw metadata/member.user so autoplay entries keep their origin user.
 */
function resolveRequester(song) {
    if (!song) return null;
    return (song.metadata?.requesterUser && !song.metadata.requesterUser.bot ? song.metadata.requesterUser : null)
        || (song.member?.user && !song.member.user.bot ? song.member.user : null)
        || (song.user && !song.user.bot ? song.user : null)
        || song.metadata?.requesterUser
        || song.member?.user
        || song.user
        || null;
}

/**
 * Display name for the requester: guild nickname > global name > username.
 */
function requesterName(song, user = resolveRequester(song)) {
    return song?.member?.displayName || user?.globalName || user?.username || null;
}

module.exports = { resolveRequester, requesterName };
