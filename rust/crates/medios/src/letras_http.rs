use super::letras::{elegir_mejor, ItemLetraLrc, ResultadoLetras, LRCLIB_API};
use std::time::Duration;

pub async fn peticion_lrc(
    client: &reqwest::Client,
    params: &[(&str, &str)],
) -> Result<Option<Vec<ItemLetraLrc>>, String> {
    let url = format!("{LRCLIB_API}/search");
    let resp = client
        .get(&url)
        .header(
            "User-Agent",
            "MusicBot (https://github.com/umutxyp/musicbot)",
        )
        .query(params)
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().as_u16() == 404 {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(format!("LRCLIB responded with {}", resp.status()));
    }
    let items: Vec<ItemLetraLrc> = resp.json().await.map_err(|e| e.to_string())?;
    Ok(Some(items))
}

pub async fn buscar_lrclib_http(
    client: &reqwest::Client,
    title: &str,
    artist: &str,
    duracion: u64,
) -> Result<Option<ResultadoLetras>, String> {
    let mut results = if !artist.is_empty() {
        peticion_lrc(client, &[("track_name", title), ("artist_name", artist)]).await?
    } else {
        None
    };

    if results.as_ref().map(|r| r.is_empty()).unwrap_or(true) {
        let q = if artist.is_empty() {
            title.to_string()
        } else {
            format!("{artist} {title}")
        };
        results = peticion_lrc(client, &[("q", &q)]).await?;
    }

    let items = results.unwrap_or_default();
    let best = elegir_mejor(&items, duracion);
    Ok(best.map(|b| ResultadoLetras {
        title: b.track_name.unwrap_or_else(|| title.to_string()),
        artist: b.artist_name.unwrap_or_else(|| artist.to_string()),
        lyrics: b.plain_lyrics.unwrap_or_default().trim().to_string(),
        source: "LRCLIB".to_string(),
    }))
}
