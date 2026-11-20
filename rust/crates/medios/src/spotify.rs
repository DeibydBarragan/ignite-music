use crate::modelos::{Cancion, FuentesError, ListaCanciones, ResultadoResolucion};
use crate::spotify_modelos::TokenResponse;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub use crate::spotify_modelos::{
    create_search_query, parse_spotify_url, spotify_track_to_song, SpotifyUrlType,
};

#[derive(Clone)]
pub struct SpotifyClient {
    client_id: String,
    client_secret: String,
    token: Arc<Mutex<Option<(String, u64)>>>,
    http: reqwest::Client,
}

impl SpotifyClient {
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self {
            client_id,
            client_secret,
            token: Arc::new(Mutex::new(None)),
            http: reqwest::Client::builder().build().unwrap_or_default(),
        }
    }

    pub fn has_credentials(&self) -> bool {
        !self.client_id.trim().is_empty() && !self.client_secret.trim().is_empty()
    }

    async fn get_token(&self) -> Result<String, FuentesError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        {
            let guard = self.token.lock().unwrap();
            if let Some((tok, exp)) = guard.as_ref() {
                if now + 60_000 < *exp {
                    return Ok(tok.clone());
                }
            }
        }

        if !self.has_credentials() {
            return Err(FuentesError::Spotify(
                "Spotify client credentials not configured".to_string(),
            ));
        }

        let params = [
            ("grant_type", "client_credentials"),
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
        ];

        let resp = self
            .http
            .post("https://accounts.spotify.com/api/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| FuentesError::Spotify(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(FuentesError::Spotify(format!(
                "Token request failed with status {}",
                resp.status()
            )));
        }

        let body: TokenResponse = resp
            .json()
            .await
            .map_err(|e| FuentesError::Spotify(e.to_string()))?;

        let expires_at = now + body.expires_in * 1000;
        let mut guard = self.token.lock().unwrap();
        *guard = Some((body.access_token.clone(), expires_at));
        Ok(body.access_token)
    }

    pub async fn resolve(&self, url: &str) -> Result<ResultadoResolucion, FuentesError> {
        let parsed = parse_spotify_url(url).ok_or(FuentesError::UrlNoSoportada)?;
        let token = self.get_token().await?;

        match parsed {
            SpotifyUrlType::Track(id) => {
                let ep = format!("https://api.spotify.com/v1/tracks/{id}");
                let val: serde_json::Value = self
                    .http
                    .get(&ep)
                    .bearer_auth(&token)
                    .send()
                    .await
                    .map_err(|e| FuentesError::Spotify(e.to_string()))?
                    .json()
                    .await
                    .map_err(|e| FuentesError::Spotify(e.to_string()))?;

                Ok(ResultadoResolucion::Cancion(spotify_track_to_song(&val)))
            }
            SpotifyUrlType::Album(id) => {
                let ep = format!("https://api.spotify.com/v1/albums/{id}");
                let val: serde_json::Value = self
                    .http
                    .get(&ep)
                    .bearer_auth(&token)
                    .send()
                    .await
                    .map_err(|e| FuentesError::Spotify(e.to_string()))?
                    .json()
                    .await
                    .map_err(|e| FuentesError::Spotify(e.to_string()))?;

                let name = val["name"].as_str().unwrap_or("Spotify Album").to_string();
                let thumb = val["images"][0]["url"].as_str().map(ToString::to_string);
                let items = val["tracks"]["items"]
                    .as_array()
                    .ok_or(FuentesError::ListaVacia)?;
                let total = items.len();
                let songs: Vec<Cancion> = items
                    .iter()
                    .take(100)
                    .map(|it| {
                        let mut s = spotify_track_to_song(it);
                        if s.miniatura.is_none() {
                            s.miniatura = thumb.clone();
                        }
                        s
                    })
                    .collect();

                if songs.is_empty() {
                    return Err(FuentesError::ListaVacia);
                }

                Ok(ResultadoResolucion::Lista(ListaCanciones {
                    fuente: "spotify".to_string(),
                    id: id.clone(),
                    nombre: name,
                    url: format!("https://open.spotify.com/album/{id}"),
                    miniatura: thumb,
                    canciones: songs,
                    truncada: total > 100,
                }))
            }
            SpotifyUrlType::Playlist(id) => {
                let ep = format!("https://api.spotify.com/v1/playlists/{id}");
                let val: serde_json::Value = self
                    .http
                    .get(&ep)
                    .bearer_auth(&token)
                    .send()
                    .await
                    .map_err(|e| FuentesError::Spotify(e.to_string()))?
                    .json()
                    .await
                    .map_err(|e| FuentesError::Spotify(e.to_string()))?;

                let name = val["name"]
                    .as_str()
                    .unwrap_or("Spotify Playlist")
                    .to_string();
                let thumb = val["images"][0]["url"].as_str().map(ToString::to_string);
                let items = val["tracks"]["items"]
                    .as_array()
                    .ok_or(FuentesError::ListaVacia)?;
                let total = items.len();
                let mut songs = Vec::new();
                for item in items.iter().take(100) {
                    if let Some(track) = item.get("track") {
                        if !track.is_null() {
                            songs.push(spotify_track_to_song(track));
                        }
                    }
                }
                if songs.is_empty() {
                    return Err(FuentesError::ListaVacia);
                }

                Ok(ResultadoResolucion::Lista(ListaCanciones {
                    fuente: "spotify".to_string(),
                    id: id.clone(),
                    nombre: name,
                    url: format!("https://open.spotify.com/playlist/{id}"),
                    miniatura: thumb,
                    canciones: songs,
                    truncada: total > 100,
                }))
            }
        }
    }
}
