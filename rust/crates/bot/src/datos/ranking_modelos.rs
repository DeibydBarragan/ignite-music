//! Modelos y agregación de ranking de canciones (Spec S-30).

use super::historial::{ExclusionesHistorial, RegistroReproduccion};
use crate::api::formas::FALLBACK_COVER;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const TOP_SONGS_LIMIT: usize = 50;
pub const SCAN_LIMIT: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CancionRanking {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album_art: String,
    pub cover_url: Option<String>,
    pub duration: u32,
    pub formatted_duration: String,
    pub source: String,
    pub url: String,
    pub category: String,
    pub is_lyric: bool,
    pub is_live: bool,
    pub is_autoplay: bool,
    pub play_count: u64,
    pub rank: usize,
    pub last_played: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CancionAgregada {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album_art: Option<String>,
    pub url: String,
    pub duration: u32,
    pub source: String,
    pub play_count: u64,
    pub last_played: Option<String>,
}

pub fn complete_song(item: CancionAgregada, rank: usize) -> CancionRanking {
    let dur = item.duration;
    let url = item.url;
    let id = if !item.id.is_empty() {
        item.id
    } else {
        url.clone()
    };
    let cover_url = item.album_art;
    let cover = cover_url
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(FALLBACK_COVER)
        .to_owned();
    let title = if !item.title.is_empty() {
        item.title
    } else {
        "Desconocido".into()
    };
    let artist = if !item.artist.is_empty() {
        item.artist
    } else {
        "Desconocido".into()
    };
    let source = if item.source == "spotify" {
        "spotify".into()
    } else {
        "youtube".into()
    };

    CancionRanking {
        id,
        title,
        artist,
        album_art: cover,
        cover_url,
        duration: dur,
        formatted_duration: format!("{}:{:02}", dur / 60, dur % 60),
        source,
        url,
        category: "hits".into(),
        is_lyric: false,
        is_live: false,
        is_autoplay: false,
        play_count: item.play_count,
        rank,
        last_played: item.last_played.filter(|fecha| !fecha.is_empty()),
    }
}

pub fn aggregate_plays(
    records: &[RegistroReproduccion],
    limit: usize,
    exclusiones: &ExclusionesHistorial,
) -> Vec<CancionRanking> {
    let mut map: HashMap<String, CancionAgregada> = HashMap::new();
    for item in records {
        if exclusiones.es_excluido(&item.played_at) {
            continue;
        }
        let key = if !item.url.is_empty() {
            &item.url
        } else if !item.song_id.is_empty() {
            &item.song_id
        } else {
            &item.title
        };
        if key.is_empty() {
            continue;
        }

        let entry = map.entry(key.clone()).or_insert_with(|| CancionAgregada {
            id: item.song_id.clone(),
            title: item.title.clone(),
            artist: item.artist.clone(),
            album_art: item.album_art.clone(),
            url: item.url.clone(),
            duration: item.duration,
            source: item.source.clone(),
            play_count: 0,
            last_played: Some(item.played_at.clone()),
        });
        entry.play_count += 1;
        if let Some(actual) = &entry.last_played {
            if item.played_at > *actual {
                entry.last_played = Some(item.played_at.clone());
            }
        } else {
            entry.last_played = Some(item.played_at.clone());
        }
    }

    let mut lista: Vec<CancionAgregada> = map.into_values().collect();
    lista.sort_by(|a, b| {
        b.play_count
            .cmp(&a.play_count)
            .then_with(|| b.last_played.cmp(&a.last_played))
    });

    lista
        .into_iter()
        .take(limit)
        .enumerate()
        .map(|(idx, item)| complete_song(item, idx + 1))
        .collect()
}
