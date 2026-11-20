use super::letras_http::buscar_lrclib_http;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub const LRCLIB_API: &str = "https://lrclib.net/api";
pub const LYRICS_PAGE_SIZE: usize = 2800;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultadoLetras {
    pub title: String,
    pub artist: String,
    pub lyrics: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemLetraLrc {
    #[serde(rename = "trackName")]
    pub track_name: Option<String>,
    #[serde(rename = "artistName")]
    pub artist_name: Option<String>,
    #[serde(rename = "plainLyrics")]
    pub plain_lyrics: Option<String>,
    #[serde(default)]
    pub instrumental: bool,
    #[serde(default)]
    pub duration: f64,
}

pub fn limpiar_titulo(title: &str, artist: &str) -> (String, String) {
    let re_brackets = Regex::new(r"\[[^\]]*\]|\([^)]*\)|【[^】]*】").unwrap();
    let re_words = Regex::new(
        r"(?i)\b(official|music|lyric|lyrics|audio|video|visualizer|hd|4k|mv|remastered)\b",
    )
    .unwrap();
    let re_feat = Regex::new(r"(?i)\s(ft|feat)\.?\s.*$").unwrap();
    let re_dash = Regex::new(r"^(.+?)\s+[-–—]\s+(.+)$").unwrap();
    let re_topic = Regex::new(r"(?i)\s*-\s*Topic$").unwrap();
    let re_vevo = Regex::new(r"(?i)VEVO$").unwrap();

    let val = re_brackets.replace_all(title, " ");
    let val = re_words.replace_all(&val, " ");
    let val = re_feat.replace_all(&val, " ");
    let mut val = val.split_whitespace().collect::<Vec<_>>().join(" ");

    let guessed = re_topic.replace_all(artist, "");
    let mut guessed_artist = re_vevo.replace_all(&guessed, "").trim().to_string();

    if let Some(caps) = re_dash.captures(&val) {
        if let (Some(a), Some(t)) = (caps.get(1), caps.get(2)) {
            guessed_artist = a.as_str().trim().to_string();
            val = t.as_str().trim().to_string();
        }
    }

    let final_title = if val.is_empty() {
        title.trim().to_string()
    } else {
        val
    };
    (final_title, guessed_artist)
}

pub fn elegir_mejor(results: &[ItemLetraLrc], duration: u64) -> Option<ItemLetraLrc> {
    let mut with_lyrics: Vec<&ItemLetraLrc> = results
        .iter()
        .filter(|it| {
            it.plain_lyrics
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false)
                && !it.instrumental
        })
        .collect();
    if with_lyrics.is_empty() {
        return None;
    }
    if duration == 0 {
        return Some(with_lyrics[0].clone());
    }
    let dur_f = duration as f64;
    with_lyrics.sort_by(|a, b| {
        (a.duration - dur_f)
            .abs()
            .partial_cmp(&(b.duration - dur_f).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Some(with_lyrics[0].clone())
}

pub fn dividir_letras(lyrics: &str) -> Vec<String> {
    let re_para = Regex::new(r"\n{2,}").unwrap();
    let mut pages = Vec::new();
    let mut current = String::new();

    for paragraph in re_para.split(lyrics) {
        let block = paragraph.trim();
        if block.is_empty() {
            continue;
        }
        let b_len = block.chars().count();
        let c_len = current.chars().count();
        if !current.is_empty() && c_len + b_len + 2 > LYRICS_PAGE_SIZE {
            pages.push(current);
            current = String::new();
        }
        if b_len > LYRICS_PAGE_SIZE {
            let chars: Vec<char> = block.chars().collect();
            for chunk in chars.chunks(LYRICS_PAGE_SIZE) {
                pages.push(chunk.iter().collect());
            }
            continue;
        }
        if current.is_empty() {
            current = block.to_string();
        } else {
            current.push_str("\n\n");
            current.push_str(block);
        }
    }
    if !current.is_empty() {
        pages.push(current);
    }
    if pages.is_empty() {
        vec![String::new()]
    } else {
        pages
    }
}

type EntradaCacheLetra = (Option<ResultadoLetras>, Instant);

#[derive(Clone)]
pub struct CacheLetras {
    entries: Arc<RwLock<HashMap<String, EntradaCacheLetra>>>,
    ttl: Duration,
    max_size: usize,
}

impl Default for CacheLetras {
    fn default() -> Self {
        Self::nuevo(Duration::from_secs(6 * 3600), 500)
    }
}

impl CacheLetras {
    pub fn nuevo(ttl: Duration, max_size: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            ttl,
            max_size,
        }
    }

    pub async fn obtener(&self, key: &str) -> Option<Option<ResultadoLetras>> {
        let map = self.entries.read().await;
        if let Some((res, exp)) = map.get(key) {
            if Instant::now() < *exp {
                return Some(res.clone());
            }
        }
        None
    }

    pub async fn insertar(&self, key: String, res: Option<ResultadoLetras>) {
        let mut map = self.entries.write().await;
        if map.len() >= self.max_size {
            map.clear();
        }
        map.insert(key, (res, Instant::now() + self.ttl));
    }
}

pub async fn buscar_letras(
    client: &reqwest::Client,
    cancion_nombre: &str,
    cancion_artista: &str,
    duracion: u64,
    cache: Option<&CacheLetras>,
) -> Result<Option<ResultadoLetras>, String> {
    let (title, artist) = limpiar_titulo(cancion_nombre, cancion_artista);
    if title.is_empty() {
        return Ok(None);
    }
    let cache_key = format!("{title}|{artist}").to_lowercase();
    if let Some(c) = cache {
        if let Some(cached) = c.obtener(&cache_key).await {
            return Ok(cached);
        }
    }

    let res = buscar_lrclib_http(client, &title, &artist, duracion).await;
    if let Ok(ref val) = res {
        if let Some(c) = cache {
            c.insertar(cache_key, val.clone()).await;
        }
    }
    res
}
