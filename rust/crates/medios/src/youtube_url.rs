use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

static RE_PATH_VIDEO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^/(?:shorts|live|embed|v)/([\w-]{11})").unwrap());

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum YouTubeUrlType {
    Video { id: String },
    Playlist { id: String },
}

pub fn is_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn is_playlist_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn watch_url(id: &str) -> String {
    format!("https://www.youtube.com/watch?v={id}")
}

pub fn thumbnail_for(id: &str) -> String {
    format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg")
}

pub fn parse_youtube_url(input: &str) -> Option<YouTubeUrlType> {
    let url = url::Url::parse(input).ok()?;
    let host = url.host_str()?.to_lowercase();
    let is_yt = matches!(
        host.as_str(),
        "youtube.com"
            | "www.youtube.com"
            | "m.youtube.com"
            | "music.youtube.com"
            | "youtu.be"
            | "www.youtu.be"
            | "youtube-nocookie.com"
            | "www.youtube-nocookie.com"
    );
    if !is_yt {
        return None;
    }

    let mut list = None;
    let mut video_id = None;
    for (k, v) in url.query_pairs() {
        if k == "list" {
            list = Some(v.to_string());
        } else if k == "v" {
            video_id = Some(v.to_string());
        }
    }

    if host.ends_with("youtu.be") {
        if let Some(first_seg) = url.path().trim_start_matches('/').split('/').next() {
            if !first_seg.is_empty() {
                video_id = Some(first_seg.to_string());
            }
        }
    }

    if let Some(caps) = RE_PATH_VIDEO.captures(url.path()) {
        if let Some(m) = caps.get(1) {
            video_id = Some(m.as_str().to_string());
        }
    }

    let has_video = video_id.as_deref().is_some_and(is_video_id);
    let is_mix = list.as_deref().is_some_and(|l| l.starts_with("RD"));

    if let Some(l) = list {
        if is_playlist_id(&l) && !(is_mix && has_video) {
            return Some(YouTubeUrlType::Playlist { id: l });
        }
    }

    if has_video {
        return Some(YouTubeUrlType::Video {
            id: video_id.unwrap(),
        });
    }

    None
}
