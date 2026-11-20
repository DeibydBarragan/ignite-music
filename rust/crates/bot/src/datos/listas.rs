//! Gestión y sincronización de playlists de usuario (Spec S-30).

use super::listas_modelos::*;
use super::supabase::ClienteSupabaseRest;
use crate::api::formas::FALLBACK_COVER;
use serde_json::{json, Value};
use std::sync::Arc;

pub use super::listas_modelos::{
    derive_name, detect_source, ResultadoUpsertPlaylist, UserPlaylist, UserPlaylistInput,
    MAX_USER_PLAYLISTS,
};

pub struct GestorListas {
    cliente: Option<Arc<ClienteSupabaseRest>>,
}

impl GestorListas {
    pub fn nuevo(cliente: Option<Arc<ClienteSupabaseRest>>) -> Self {
        Self { cliente }
    }

    pub async fn fetch_user_playlists(
        &self,
        user_id: &str,
        limit: usize,
    ) -> Result<Vec<UserPlaylist>, (u16, &'static str, String)> {
        let cliente = match &self.cliente {
            Some(c) => c,
            None => return Err((503, "DB_UNAVAILABLE", "Database unavailable".into())),
        };

        let capped = limit.clamp(1, MAX_USER_PLAYLISTS);
        let ruta = format!(
            "rest/v1/user_playlists?user_id=eq.{user_id}&order=created_at.desc&limit={capped}"
        );
        let resp = cliente
            .get(&ruta)
            .send()
            .await
            .map_err(|e| (500, "PLAYLIST_FETCH_FAILED", e.to_string()))?;

        if !resp.status().is_success() {
            let msg = resp.text().await.unwrap_or_default();
            return Err((500, "PLAYLIST_FETCH_FAILED", msg));
        }

        let filas = resp.json::<Vec<Value>>().await.unwrap_or_default();
        Ok(filas.iter().map(map_row).collect())
    }

    pub async fn upsert_user_playlist(
        &self,
        input: &UserPlaylistInput,
    ) -> Result<ResultadoUpsertPlaylist, (u16, &'static str, String)> {
        let cliente = match &self.cliente {
            Some(c) => c,
            None => return Err((503, "DB_UNAVAILABLE", "Database unavailable".into())),
        };

        let clean_url = input.url.trim().to_string();
        let name = derive_name(&clean_url, input.name.as_deref());
        let source = input
            .source
            .clone()
            .unwrap_or_else(|| detect_source(&clean_url));
        let cover = input.cover.clone().unwrap_or_else(|| {
            input
                .tracks
                .first()
                .and_then(|t| t.get("albumArt"))
                .and_then(Value::as_str)
                .unwrap_or(FALLBACK_COVER)
                .to_string()
        });

        let payload = json!({
            "user_id": input.user_id,
            "name": name,
            "source": source,
            "url": clean_url,
            "cover": cover,
            "tracks": input.tracks,
            "synced_at": input.synced_at,
        });

        let r_find = format!(
            "rest/v1/user_playlists?user_id=eq.{}&url=eq.{}&select=id&limit=1",
            input.user_id, clean_url
        );
        let existing_id: Option<String> = match cliente.get(&r_find).send().await {
            Ok(resp) if resp.status().is_success() => resp
                .json::<Vec<Value>>()
                .await
                .ok()
                .and_then(|mut v| v.pop())
                .and_then(|obj| obj.get("id").and_then(Value::as_str).map(String::from)),
            _ => None,
        };

        if let Some(id) = existing_id {
            let r_update = format!("rest/v1/user_playlists?id=eq.{id}");
            let resp = cliente
                .patch(&r_update)
                .map_err(|e| (500, "PLAYLIST_UPDATE_FAILED", e.to_string()))?
                .header("Prefer", "return=representation")
                .json(&payload)
                .send()
                .await
                .map_err(|e| (500, "PLAYLIST_UPDATE_FAILED", e.to_string()))?;

            if !resp.status().is_success() {
                let msg = resp.text().await.unwrap_or_default();
                return Err((500, "PLAYLIST_UPDATE_FAILED", msg));
            }
            let row = resp
                .json::<Vec<Value>>()
                .await
                .ok()
                .and_then(|mut v| v.pop())
                .unwrap_or(payload);
            Ok(ResultadoUpsertPlaylist {
                playlist: map_row(&row),
                created: false,
            })
        } else {
            let resp = cliente
                .post("rest/v1/user_playlists")
                .map_err(|e| (500, "PLAYLIST_INSERT_FAILED", e.to_string()))?
                .header("Prefer", "return=representation")
                .json(&payload)
                .send()
                .await
                .map_err(|e| (500, "PLAYLIST_INSERT_FAILED", e.to_string()))?;

            if !resp.status().is_success() {
                let msg = resp.text().await.unwrap_or_default();
                return Err((500, "PLAYLIST_INSERT_FAILED", msg));
            }
            let row = resp
                .json::<Vec<Value>>()
                .await
                .ok()
                .and_then(|mut v| v.pop())
                .unwrap_or(payload);
            Ok(ResultadoUpsertPlaylist {
                playlist: map_row(&row),
                created: true,
            })
        }
    }
}
