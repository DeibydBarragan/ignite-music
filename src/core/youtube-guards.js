'use strict';

/**
 * Guards shared by every playback entry point (Phase 2.3): the bot-check
 * detection used by the extractor and the error mapper, and the "supported
 * URL" assertion used by manager, playlists and the REST API ops.
 */

const { DisTubeError } = /** @type {any} */ (require('distube'));

const SOUNDCLOUD_URL = /soundcloud\.com/i;
const BOT_CHECK = /sign in to confirm|not a bot|confirm you.re not/i;

const UNSUPPORTED_SOUNDCLOUD = 'SoundCloud is no longer supported. Use a YouTube or Spotify link, or search by name.';

/** True when the input targets SoundCloud (no longer supported by any plugin). */
function isSoundCloudUrl(input) {
    return SOUNDCLOUD_URL.test(String(input || ''));
}

/** Error message when the input is an unsupported URL, or null when accepted. */
function unsupportedUrlReason(input) {
    return isSoundCloudUrl(input) ? UNSUPPORTED_SOUNDCLOUD : null;
}

/**
 * Throw DisTubeError(NOT_SUPPORTED_URL) so errors.js maps it to ui.unsupported;
 * speakers of the REST API should use unsupportedUrlReason() + ApiError instead.
 */
function assertSupportedUrl(input) {
    if (isSoundCloudUrl(input)) throw new DisTubeError('NOT_SUPPORTED_URL', UNSUPPORTED_SOUNDCLOUD);
    return true;
}

/** DisTube/yt-dlp "Sign in to confirm you're not a bot" detection. */
function isBotCheck(error) {
    const text = `${error?.message || ''} ${error?.stderr || ''}`;
    return BOT_CHECK.test(text);
}

module.exports = { isSoundCloudUrl, unsupportedUrlReason, assertSupportedUrl, isBotCheck, UNSUPPORTED_SOUNDCLOUD };
