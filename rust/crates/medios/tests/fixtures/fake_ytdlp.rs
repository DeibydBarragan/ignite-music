#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if let Ok(log_path) = env::var("FAKE_YTDLP_LOG") {
        let matches_target = env::var("FAKE_YTDLP_LOG_MATCH")
            .map(|needle| args.iter().any(|arg| arg.contains(&needle)))
            .unwrap_or(true);
        if matches_target {
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(log_path) {
                let _ = writeln!(f, "{}", serde_json::to_string(&args).unwrap_or_default());
            }
        }
    }

    if args.first().map(|s| s.as_str()) == Some("--version") {
        println!("2026.09.01");
        std::process::exit(0);
    }

    if let Some(delay) = env::var("FAKE_YTDLP_DELAY_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    {
        sleep(Duration::from_millis(delay));
    }

    let target = args.last().map(|s| s.as_str()).unwrap_or("");
    if target == "slow" {
        sleep(Duration::from_secs(10));
        println!("{{}}");
    } else if target == "fail" || target.contains("fail_bot") {
        eprintln!("WARNING: something");
        eprintln!("ERROR: [youtube] abc: Sign in to confirm you’re not a bot");
        std::process::exit(1);
    } else if target == "invalid" {
        print!("not json");
    } else if target.contains("list=PL1") {
        print!(
            "{}",
            serde_json::json!({
                "id": "PL1",
                "title": "My list",
                "entries": [
                    { "id": "aaaaaaaaaaa", "title": "A", "duration": 100 },
                    { "id": "bbbbbbbbbbb", "title": "[Private video]" },
                    { "id": "ccccccccccc", "title": "C", "duration": 50 },
                    null
                ]
            })
        );
    } else if target.contains("list=PL_EMPTY") {
        print!("{}", serde_json::json!({ "id": "PL_EMPTY", "entries": [] }));
    } else if target.contains("list=RD") {
        print!(
            "{}",
            serde_json::json!({
                "entries": [
                    { "id": "dQw4w9WgXcQ", "title": "self", "duration": 200 },
                    { "id": "aaaaaaaaaaa", "title": "ok", "duration": 200 },
                    { "id": "bbbbbbbbbbb", "title": "short", "duration": 30 },
                    { "id": "ccccccccccc", "title": "mix", "duration": 3600 },
                    { "id": "ddddddddddd", "title": "unknown length" }
                ]
            })
        );
    } else if target.contains("bandcamp.com") {
        print!(
            "{}",
            serde_json::json!({
                "id": "1",
                "title": "Track",
                "extractor_key": "Bandcamp",
                "duration": 60,
                "format_id": "mp3",
                "url": "https://cdn/x.mp3",
                "webpage_url": "https://band.bandcamp.com/track/x"
            })
        );
    } else if target.contains("/album") {
        print!(
            "{}",
            serde_json::json!({
                "id": "al",
                "title": "Album",
                "extractor_key": "generic",
                "entries": [
                    { "id": "1", "title": "a", "url": "https://x/1" },
                    { "id": "2", "title": "b", "url": "https://x/2" }
                ]
            })
        );
    } else if target.starts_with("ytsearch1:") {
        print!(
            "{}",
            serde_json::json!({
                "entries": [
                    {
                        "id": "dQw4w9WgXcQ",
                        "title": "Never Gonna Give You Up",
                        "duration": 213,
                        "uploader": "Rick Astley",
                        "url": "https://rr1.googlevideo.com/videoplayback?id=123&expire=9999999999"
                    }
                ]
            })
        );
    } else if target.starts_with("ytsearch") {
        print!(
            "{}",
            serde_json::json!({
                "entries": [
                    { "id": "aaaaaaaaaaa", "title": "A", "duration": 10 },
                    { "id": "bad" }
                ]
            })
        );
    } else if target.contains("results?search_query") {
        let mut entries = vec![serde_json::json!({
            "id": "PL_FOUND", "title": "Found Playlist", "uploader": "Curator",
            "thumbnail": "https://thumb/1.jpg", "playlist_count": 37,
        })];
        if target.contains("limit+check") {
            entries.push(
                serde_json::json!({"id":"PL_SECOND","title":"Second Playlist","n_entries":2}),
            );
        }
        print!("{}", serde_json::json!({"entries":entries}));
    } else {
        eprintln!("WARNING: noisy warning that must not break JSON");
        print!(
            "{}",
            serde_json::json!({
                "id": "abcdefghijk",
                "title": "Echo",
                "target": target,
                "url": "https://rr1.googlevideo.com/videoplayback?expire=9999999999"
            })
        );
    }
}
