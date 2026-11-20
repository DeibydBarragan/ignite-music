//! Gestión de canciones favoritas de usuarios (Spec S-30).

use super::supabase::ClienteSupabaseRest;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CancionFavorito {
    pub song_id: String,
    pub title: String,
    pub artist: String,
    pub album_art: Option<String>,
    pub url: String,
    pub duration: u32,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccionFavorito {
    Agregado(String),
    Eliminado(String),
}

pub struct GestorFavoritos {
    cliente: Option<Arc<ClienteSupabaseRest>>,
}

impl GestorFavoritos {
    pub fn nuevo(cliente: Option<Arc<ClienteSupabaseRest>>) -> Self {
        Self { cliente }
    }

    pub async fn handle_favorite(
        &self,
        user_id: &str,
        song: &CancionFavorito,
    ) -> Result<AccionFavorito, (u16, &'static str, String)> {
        let cliente = match &self.cliente {
            Some(c) => c,
            None => {
                return Err((
                    503,
                    "DB_UNAVAILABLE",
                    "Base de datos no disponible temporalmente.".into(),
                ))
            }
        };

        if user_id.is_empty() {
            return Err((400, "INVALID_USER", "User ID is required".into()));
        }

        // 1. Buscar por song_id
        let r_song = format!(
            "rest/v1/user_favorites?user_id=eq.{user_id}&song_id=eq.{}&select=id&limit=1",
            song.song_id
        );
        let mut existing_id: Option<String> = match cliente.get(&r_song).send().await {
            Ok(resp) if resp.status().is_success() => resp
                .json::<Vec<Value>>()
                .await
                .ok()
                .and_then(|mut v| v.pop())
                .and_then(|obj| obj.get("id").and_then(Value::as_str).map(String::from)),
            _ => None,
        };

        // 2. Si no se encontró y hay URL, buscar por URL
        if existing_id.is_none() && !song.url.is_empty() {
            let r_url = format!(
                "rest/v1/user_favorites?user_id=eq.{user_id}&url=eq.{}&select=id&limit=1",
                song.url
            );
            if let Ok(resp) = cliente.get(&r_url).send().await {
                if resp.status().is_success() {
                    existing_id = resp
                        .json::<Vec<Value>>()
                        .await
                        .ok()
                        .and_then(|mut v| v.pop())
                        .and_then(|obj| obj.get("id").and_then(Value::as_str).map(String::from));
                }
            }
        }

        // 3. Si existe, eliminarlo
        if let Some(id) = existing_id {
            let r_del = format!("rest/v1/user_favorites?id=eq.{id}");
            let res = match cliente.delete(&r_del) {
                Ok(req) => req.send().await,
                Err(e) => return Err((500, "FAVORITE_DELETE_FAILED", e.to_string())),
            };
            match res {
                Ok(r) if r.status().is_success() => {
                    Ok(AccionFavorito::Eliminado(song.title.clone()))
                }
                Ok(r) => {
                    let err = r.text().await.unwrap_or_default();
                    error!("Error al eliminar favorito: {err}");
                    Err((500, "FAVORITE_DELETE_FAILED", err))
                }
                Err(e) => Err((500, "FAVORITE_DELETE_FAILED", e.to_string())),
            }
        } else {
            // 4. Si no existe, insertarlo
            let row = json!({
                "user_id": user_id,
                "song_id": song.song_id,
                "title": song.title,
                "artist": song.artist,
                "album_art": song.album_art,
                "url": song.url,
                "duration": song.duration,
                "source": song.source,
            });
            let req = match cliente.post("rest/v1/user_favorites") {
                Ok(req) => req,
                Err(e) => return Err((500, "FAVORITE_INSERT_FAILED", e.to_string())),
            };
            let res = req.json(&row).send().await;
            match res {
                Ok(r) if r.status().is_success() => {
                    Ok(AccionFavorito::Agregado(song.title.clone()))
                }
                Ok(r) => {
                    let err = r.text().await.unwrap_or_default();
                    error!("Error al guardar favorito: {err}");
                    Err((500, "FAVORITE_INSERT_FAILED", err))
                }
                Err(e) => Err((500, "FAVORITE_INSERT_FAILED", e.to_string())),
            }
        }
    }
}
