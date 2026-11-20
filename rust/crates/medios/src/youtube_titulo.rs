use regex::Regex;
use std::sync::LazyLock;

static RE_OFFICIAL_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\s*[\(\[]\s*(official\s*(music\s*)?video|official\s*audio|video\s*oficial|audio\s*oficial|official\s*lyric\s*video|lyric\s*video|lyrics?|visualizer)\s*[\)\]]\s*",
    )
    .unwrap()
});

static RE_SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static RE_TOPIC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(.*?)\s*-\s*topic$").unwrap());
static RE_LYRIC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\blyrics?\b").unwrap());

pub fn is_lyric_title(title: &str) -> bool {
    RE_LYRIC.is_match(title)
}

pub fn clean_title_suffix(title: &str) -> String {
    let replaced = RE_OFFICIAL_SUFFIX.replace_all(title, " ");
    RE_SPACES.replace_all(&replaced, " ").trim().to_string()
}

pub fn norm_name(value: &str) -> String {
    let lower = value.to_lowercase();
    let mut without_accents = String::with_capacity(lower.len());
    for c in lower.chars() {
        match c {
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => without_accents.push('a'),
            'é' | 'è' | 'ë' | 'ê' => without_accents.push('e'),
            'í' | 'ì' | 'ï' | 'î' => without_accents.push('i'),
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => without_accents.push('o'),
            'ú' | 'ù' | 'ü' | 'û' => without_accents.push('u'),
            'ñ' => without_accents.push('n'),
            'ç' => without_accents.push('c'),
            other => without_accents.push(other),
        }
    }

    let without_vevo = if let Some(stripped) = without_accents.strip_suffix("vevo") {
        stripped
    } else {
        &without_accents
    };

    let mut alphanumeric = String::with_capacity(without_vevo.len());
    for c in without_vevo.chars() {
        if c.is_ascii_alphanumeric() || c == ' ' {
            alphanumeric.push(c);
        }
    }

    RE_SPACES.replace_all(&alphanumeric, " ").trim().to_string()
}

pub fn split_youtube_title(raw_title: &str, uploader: Option<&str>) -> (String, String) {
    let cleaned = clean_title_suffix(raw_title);
    let channel = uploader.unwrap_or("").trim();

    if let Some(caps) = RE_TOPIC.captures(channel) {
        let topic_artist = caps.get(1).map_or("", |m| m.as_str()).trim();
        if !topic_artist.is_empty() {
            let title = if !cleaned.is_empty() {
                cleaned
            } else {
                raw_title.trim().to_string()
            };
            return (title, topic_artist.to_string());
        }
    }

    if let Some(dash) = cleaned.find(" - ") {
        if !channel.is_empty() {
            let left = cleaned[..dash].trim();
            let right = cleaned[dash + 3..].trim();
            let left_norm = norm_name(left);
            let channel_norm = norm_name(channel);

            if !right.is_empty()
                && !left_norm.is_empty()
                && !channel_norm.is_empty()
                && (left_norm.contains(&channel_norm) || channel_norm.contains(&left_norm))
            {
                return (right.to_string(), left.to_string());
            }
        }
    }

    let title = if !cleaned.is_empty() {
        cleaned
    } else {
        raw_title.trim().to_string()
    };
    (title, channel.to_string())
}

pub fn stream_ttl(url: &str) -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    stream_ttl_at(url, now)
}

pub fn stream_ttl_at(url: &str, now_ms: u64) -> u64 {
    if let Ok(parsed) = url::Url::parse(url) {
        if let Some((_, val)) = parsed.query_pairs().find(|(k, _)| k == "expire") {
            if let Ok(expire_sec) = val.parse::<u64>() {
                if expire_sec > 0 {
                    let expire_ms = expire_sec.saturating_mul(1000);
                    let diff = expire_ms
                        .saturating_sub(now_ms)
                        .saturating_sub(30 * 60 * 1000);
                    return diff.clamp(60_000, 4 * 3600 * 1000);
                }
            }
        }
    }
    3_600_000
}
