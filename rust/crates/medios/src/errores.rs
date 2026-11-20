use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

static RE_BOT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)sign in to confirm|not a bot|confirm you.re not").unwrap());
static RE_AGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)age.?restrict|confirm your age|inappropriate for some users").unwrap()
});
static RE_GEO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)available in your country|geo.?restrict|blocked it in your country").unwrap()
});
static RE_RATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b429\b|too many requests|rate.?limit").unwrap());
static RE_UNAVAILABLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)video unavailable|private video|has been removed|does not exist|is not available|no longer available",
    )
    .unwrap()
});
static RE_NETWORK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)econnreset|etimedout|enotfound|eai_again|socket hang up|network|timed out")
        .unwrap()
});
static RE_DRM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)drm|drm protected").unwrap());
static RE_STREAM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)ffmpeg|403|forbidden").unwrap());

pub fn is_bot_check(text: &str) -> bool {
    RE_BOT.is_match(text)
}

pub fn error_key(code: Option<&str>, message: Option<&str>, stderr: Option<&str>) -> &'static str {
    let c = code.unwrap_or("").trim();
    let text = format!("{} {}", message.unwrap_or(""), stderr.unwrap_or("")).to_lowercase();

    if c == "VOICE_FULL" {
        return "ui.voice_full";
    }
    if c == "VOICE_MISSING_PERMS" {
        return "errors.voice_no_permission";
    }
    if c == "VOICE_CONNECT_FAILED" || c == "VOICE_RECONNECT_FAILED" {
        return "musicplayer.failed_connect_voice";
    }
    if c == "NON_NSFW" {
        return "errors.youtube_age_restricted";
    }
    if c == "NOT_SUPPORTED_URL" || c == "NOT_SUPPORTED_SONG" {
        return "ui.unsupported";
    }
    if c == "EMPTY_PLAYLIST" || c == "EMPTY_FILTERED_PLAYLIST" {
        return "youtube.no_valid_tracks";
    }
    if c == "NO_RESULT" {
        return "musicplayer.no_results_found";
    }
    if is_bot_check(&text) {
        return "errors.youtube_bot_detection";
    }
    if RE_AGE.is_match(&text) {
        return "errors.youtube_age_restricted";
    }
    if RE_GEO.is_match(&text) {
        return "errors.youtube_geo_blocked";
    }
    if RE_RATE.is_match(&text) {
        return "errors.rate_limited";
    }
    if RE_UNAVAILABLE.is_match(&text) {
        return "errors.youtube_unavailable";
    }
    if RE_NETWORK.is_match(&text) {
        return "errors.network_error";
    }
    if c == "DRM_PROTECTED" || RE_DRM.is_match(&text) {
        return "errors.drm_protected";
    }
    if c == "FFMPEG_EXITED"
        || c == "NO_STREAM_URL"
        || c == "CANNOT_GET_STREAM_URL"
        || RE_STREAM.is_match(&text)
    {
        return "errors.stream_failed";
    }

    "errors.unknown"
}

#[derive(Debug, Clone)]
pub struct YoutubeSearchErrorState {
    pub last_search_error: Option<String>,
    pub last_search_error_at: u64,
}

pub fn describe_error(
    key: &'static str,
    message: Option<&str>,
    stderr: Option<&str>,
    youtube: Option<&YoutubeSearchErrorState>,
    since: u64,
) -> &'static str {
    if key == "errors.drm_protected" {
        return key;
    }

    let is_not_found = key == "musicplayer.no_results_found" || key == "errors.unknown";
    if is_not_found {
        if let Some(yt) = youtube {
            if let Some(cause) = &yt.last_search_error {
                if yt.last_search_error_at >= since {
                    let text = format!("{} {}", message.unwrap_or(""), stderr.unwrap_or(""))
                        .to_lowercase();
                    if !RE_DRM.is_match(&text) && cause != "errors.unknown" {
                        // Return the static string matching cause
                        return match cause.as_str() {
                            "errors.youtube_bot_detection" => "errors.youtube_bot_detection",
                            "errors.youtube_age_restricted" => "errors.youtube_age_restricted",
                            "errors.youtube_geo_blocked" => "errors.youtube_geo_blocked",
                            "errors.rate_limited" => "errors.rate_limited",
                            "errors.youtube_unavailable" => "errors.youtube_unavailable",
                            "errors.network_error" => "errors.network_error",
                            "errors.stream_failed" => "errors.stream_failed",
                            _ => key,
                        };
                    }
                }
            }
        }
    }

    key
}

pub const HINT_THROTTLE_MS: u64 = 10 * 60 * 1000;

pub fn host_hint(
    key: &str,
    now: u64,
    last_hint: &mut HashMap<String, u64>,
) -> Option<&'static str> {
    let hint = match key {
        "errors.youtube_bot_detection" => {
            "YouTube is blocking this server (\"Sign in to confirm you're not a bot\"). \
             Export YouTube cookies to cookies.txt and set COOKIES_FILE=./cookies.txt in .env \
             (or YOUTUBE_PO_TOKEN / PROXY_URL). See README.md, section \"YouTube: Sign in to confirm you're not a bot\"."
        }
        "errors.youtube_age_restricted" => {
            "Age-restricted videos need a logged-in YouTube account. \
             Set COOKIES_FILE=./cookies.txt in .env. See README.md."
        }
        "errors.rate_limited" => {
            "A platform is rate limiting this server. \
             If it keeps happening set COOKIES_FILE or PROXY_URL in .env. See README.md."
        }
        _ => return None,
    };

    let previous = last_hint.get(key).copied().unwrap_or(0);
    if now.saturating_sub(previous) < HINT_THROTTLE_MS {
        return None;
    }

    last_hint.insert(key.to_string(), now);
    Some(hint)
}
