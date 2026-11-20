'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { errorKey } = require('../src/music/errors');

const targetPath = path.join(
    __dirname,
    '..',
    'rust',
    'crates',
    'medios',
    'tests',
    'data',
    'errores-casos.json',
);

const cases = [
    { name: 'voice_full', code: 'VOICE_FULL', message: '', stderr: '' },
    { name: 'voice_missing_perms', code: 'VOICE_MISSING_PERMS', message: '', stderr: '' },
    { name: 'voice_connect_failed', code: 'VOICE_CONNECT_FAILED', message: '', stderr: '' },
    { name: 'voice_reconnect_failed', code: 'VOICE_RECONNECT_FAILED', message: '', stderr: '' },
    { name: 'non_nsfw', code: 'NON_NSFW', message: '', stderr: '' },
    { name: 'not_supported_url', code: 'NOT_SUPPORTED_URL', message: '', stderr: '' },
    { name: 'not_supported_song', code: 'NOT_SUPPORTED_SONG', message: '', stderr: '' },
    { name: 'empty_playlist', code: 'EMPTY_PLAYLIST', message: '', stderr: '' },
    { name: 'empty_filtered_playlist', code: 'EMPTY_FILTERED_PLAYLIST', message: '', stderr: '' },
    { name: 'no_result', code: 'NO_RESULT', message: '', stderr: '' },
    { name: 'drm_protected_code', code: 'DRM_PROTECTED', message: '', stderr: '' },
    { name: 'ffmpeg_exited', code: 'FFMPEG_EXITED', message: '', stderr: '' },
    { name: 'no_stream_url', code: 'NO_STREAM_URL', message: '', stderr: '' },
    { name: 'cannot_get_stream_url', code: 'CANNOT_GET_STREAM_URL', message: '', stderr: '' },

    // Bot check text
    { name: 'bot_check_sign_in', code: '', message: 'Sign in to confirm you are not a bot', stderr: '' },
    { name: 'bot_check_not_a_bot', code: '', message: 'Please prove you are not a bot', stderr: '' },
    { name: 'bot_check_confirm_you_re_not', code: '', message: "Confirm you're not a robot", stderr: '' },
    { name: 'bot_check_in_stderr', code: '', message: 'Failed', stderr: 'ERROR: Sign in to confirm you are not a bot' },

    // Age restricted text
    { name: 'age_restrict_hyphen', code: '', message: 'This video is age-restricted', stderr: '' },
    { name: 'age_restrict_space', code: '', message: 'Age restricted content', stderr: '' },
    { name: 'age_restrict_confirm_age', code: '', message: 'Please confirm your age to watch', stderr: '' },
    { name: 'age_restrict_inappropriate', code: '', message: 'May be inappropriate for some users', stderr: '' },

    // Geo blocked text
    { name: 'geo_not_available_country', code: '', message: 'The video is not available in your country', stderr: '' },
    { name: 'geo_restrict_hyphen', code: '', message: 'Video is geo-restricted by copyright owner', stderr: '' },
    { name: 'geo_blocked_it', code: '', message: 'The uploader has blocked it in your country', stderr: '' },

    // Rate limited text
    { name: 'rate_limit_429_word', code: '', message: 'HTTP 429 response from server', stderr: '' },
    { name: 'rate_limit_too_many', code: '', message: 'Too many requests received', stderr: '' },
    { name: 'rate_limit_words', code: '', message: 'Rate limit has been exceeded', stderr: '' },
    { name: 'rate_limit_hyphen', code: '', message: 'Client hit rate-limit', stderr: '' },

    // Video unavailable text
    { name: 'unavailable_video_unavailable', code: '', message: 'Video unavailable', stderr: '' },
    { name: 'unavailable_private', code: '', message: 'This is a private video', stderr: '' },
    { name: 'unavailable_removed', code: '', message: 'Video has been removed', stderr: '' },
    { name: 'unavailable_does_not_exist', code: '', message: 'The requested resource does not exist', stderr: '' },
    { name: 'unavailable_is_not_available', code: '', message: 'Content is not available', stderr: '' },
    { name: 'unavailable_no_longer', code: '', message: 'This video is no longer available', stderr: '' },

    // Network errors text
    { name: 'network_econnreset', code: '', message: 'Connection error ECONNRESET', stderr: '' },
    { name: 'network_etimedout', code: '', message: 'Socket connection ETIMEDOUT', stderr: '' },
    { name: 'network_enotfound', code: '', message: 'Lookup failed: ENOTFOUND', stderr: '' },
    { name: 'network_eai_again', code: '', message: 'DNS failure EAI_AGAIN', stderr: '' },
    { name: 'network_socket_hang_up', code: '', message: 'Error: socket hang up', stderr: '' },
    { name: 'network_word', code: '', message: 'Temporary network failure', stderr: '' },
    { name: 'network_timed_out', code: '', message: 'Process timed out after 30s', stderr: '' },

    // DRM text
    { name: 'drm_word', code: '', message: 'Stream requires drm license', stderr: '' },
    { name: 'drm_protected_phrase', code: '', message: 'This stream is drm protected', stderr: '' },

    // Stream failed text
    { name: 'stream_ffmpeg_text', code: '', message: 'ffmpeg error while decoding', stderr: '' },
    { name: 'stream_403', code: '', message: 'HTTP Error 403: Forbidden', stderr: '' },
    { name: 'stream_forbidden', code: '', message: 'Access forbidden by upstream', stderr: '' },

    // Precedence tests
    { name: 'precedence_code_over_text', code: 'VOICE_FULL', message: 'HTTP 403 forbidden', stderr: '' },
    { name: 'precedence_bot_over_age', code: '', message: 'Sign in to confirm you are not a bot and confirm your age', stderr: '' },
    { name: 'precedence_drm_over_ffmpeg', code: '', message: 'ffmpeg error on drm protected media', stderr: '' },

    // Fallbacks
    { name: 'unknown_empty', code: '', message: '', stderr: '' },
    { name: 'unknown_random_text', code: '', message: 'Something completely unpredicted happened', stderr: '' },
    { name: 'unknown_unrecognized_code', code: 'SOME_WEIRD_CODE', message: 'unrelated', stderr: '' },
];

const results = cases.map((c) => {
    const errorObj = {
        code: c.code,
        message: c.message,
        stderr: c.stderr,
    };
    const key = errorKey(errorObj);
    return {
        nombre: c.name,
        codigo: c.code || null,
        mensaje: c.message || null,
        stderr: c.stderr || null,
        esperado: key,
    };
});

fs.mkdirSync(path.dirname(targetPath), { recursive: true });
fs.writeFileSync(targetPath, JSON.stringify(results, null, 2) + '\n', 'utf8');
console.log(`Generated ${results.length} error cases at ${targetPath}`);
