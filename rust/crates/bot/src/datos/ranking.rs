//! Ranking de canciones más reproducidas (Spec S-30).

use super::historial::{GestorHistorial, RegistroReproduccion};
use super::supabase::ClienteSupabaseRest;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tracing::debug;

pub use super::ranking_modelos::{
    aggregate_plays, complete_song, CancionAgregada, CancionRanking, SCAN_LIMIT, TOP_SONGS_LIMIT,
};

pub struct GestorRanking {
    cliente: Option<Arc<ClienteSupabaseRest>>,
    historial: Arc<GestorHistorial>,
}

#[derive(Deserialize)]
struct FilaRpc {
    song_id: Option<String>,
    title: Option<String>,
    artist: Option<String>,
    album_art: Option<String>,
    url: Option<String>,
    duration: Option<u32>,
    source: Option<String>,
    play_count: Option<u64>,
    last_played: Option<String>,
}

impl GestorRanking {
    pub fn nuevo(
        cliente: Option<Arc<ClienteSupabaseRest>>,
        historial: Arc<GestorHistorial>,
    ) -> Self {
        Self { cliente, historial }
    }

    pub async fn get_top_songs(
        &self,
        guild_id: &str,
        timeframe: &str,
        limit: usize,
    ) -> Vec<CancionRanking> {
        if guild_id.is_empty() {
            return Vec::new();
        }
        let capped = limit.clamp(1, TOP_SONGS_LIMIT);
        let min_time = match timeframe {
            "week" => Some((chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339()),
            "month" => Some((chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339()),
            _ => None,
        };

        if let Some(cliente) = &self.cliente {
            // 1. Intentar RPC top_songs
            let body = json!({
                "p_guild_id": guild_id,
                "p_since": min_time,
                "p_limit": capped,
                "p_excluded_dates": self.historial.exclusiones.obtener_fechas()
            });
            let rpc_res = cliente.rpc("top_songs").json(&body).send().await;
            if let Ok(resp) = rpc_res {
                if resp.status().is_success() {
                    if let Ok(filas) = resp.json::<Vec<FilaRpc>>().await {
                        return filas
                            .into_iter()
                            .take(capped)
                            .enumerate()
                            .map(|(idx, f)| {
                                complete_song(
                                    CancionAgregada {
                                        id: f.song_id.unwrap_or_default(),
                                        title: f.title.unwrap_or_default(),
                                        artist: f.artist.unwrap_or_default(),
                                        album_art: f.album_art,
                                        url: f.url.unwrap_or_default(),
                                        duration: f.duration.unwrap_or(0),
                                        source: f.source.unwrap_or_else(|| "youtube".into()),
                                        play_count: f.play_count.unwrap_or(0),
                                        last_played: f.last_played,
                                    },
                                    idx + 1,
                                )
                            })
                            .collect();
                    }
                }
            }
            debug!("top_songs RPC fallo o no disponible; intentando escaneo acotado");

            // 2. Escaneo acotado en song_plays
            let mut ruta = format!(
                "rest/v1/song_plays?guild_id=eq.{guild_id}&order=played_at.desc&limit={SCAN_LIMIT}"
            );
            if let Some(ref m) = min_time {
                ruta.push_str(&format!("&played_at=gte.{m}"));
            }
            if let Ok(resp) = cliente.get(&ruta).send().await {
                if resp.status().is_success() {
                    if let Ok(filas) = resp.json::<Vec<RegistroReproduccion>>().await {
                        if !filas.is_empty() {
                            return aggregate_plays(&filas, capped, &self.historial.exclusiones);
                        }
                    }
                }
            }
        }

        // 3. Fallback al historial local
        let local = self.historial.cargar_local(guild_id);
        let filtrado: Vec<RegistroReproduccion> = if let Some(ref m) = min_time {
            local.into_iter().filter(|r| r.played_at >= *m).collect()
        } else {
            local
        };
        aggregate_plays(&filtrado, capped, &self.historial.exclusiones)
    }

    pub async fn registrar_reproduccion(
        &self,
        guild_id: &str,
        song: &super::historial::RegistroCancionInput,
        dev_mode: bool,
    ) {
        self.historial.record_play(guild_id, song, dev_mode).await;
    }
}
