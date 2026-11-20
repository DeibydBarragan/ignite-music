//! Reproducción real de audio con yt-dlp (Spec S-13/S-16 funcional).
//!
//! `driver_reproduccion.rs` implementa la vía de reproducción que falta para
//! que el bot Rust reproduzca audio de verdad: usa `yt-dlp` como programa de
//! extracción de streams (la misma ruta que ya usan los `medios` y los
//! slash-commands para buscar/resolver) y entrega la URL de audio a songbird
//! con el proxy HTTP estándar.

use crate::driver::{MedidorInicioPista, NotificadorPista, SongbirdDriver};
use crate::driver_error::to_nz;
use crate::driver_error::DriverError;
use songbird::{
    events::{Event, EventData, TrackEvent},
    input::Input,
    tracks::{Track, TrackHandle},
};
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct OpcionesReproduccion {
    pub volumen: u8,
    pub pausada: bool,
    pub filtro: Option<String>,
    pub seek_segundos: Option<u64>,
    pub limiter: Option<medios::Limitador>,
    pub limiter_timeout_ms: u64,
    /// `true` para seek o cambio de filtro: reinstala la pista sin ser un arranque nuevo.
    pub reinicio: bool,
}

impl SongbirdDriver {
    /// Reproduce `consulta_o_url` usando `yt-dlp` como extractor de streams.
    ///
    /// `ytdlp_cmd` es la ruta al binario o el nombre en PATH. `user_args` se
    /// pasa tal cual a `yt-dlp` (por ejemplo cookies).
    pub async fn play_ytdl(
        &self,
        guild_id: u64,
        consulta_o_url: &str,
        ytdlp_cmd: &str,
        user_args: Vec<String>,
        volumen: u8,
        pausada: bool,
    ) -> Result<TrackHandle, DriverError> {
        self.play_ytdl_filtrado(
            guild_id,
            consulta_o_url,
            ytdlp_cmd,
            user_args,
            OpcionesReproduccion {
                volumen,
                pausada,
                ..Default::default()
            },
        )
        .await
    }

    pub async fn play_ytdl_filtrado(
        &self,
        guild_id: u64,
        consulta_o_url: &str,
        ytdlp_cmd: &str,
        user_args: Vec<String>,
        opciones: OpcionesReproduccion,
    ) -> Result<TrackHandle, DriverError> {
        let g_nz = to_nz(guild_id)?;
        let call_arc = self
            .songbird
            .get(g_nz)
            .ok_or(DriverError::NotInChannel(guild_id))?;

        let reinicio = opciones.reinicio;
        let input = Input::Lazy(Box::new(super::pipeline_audio::PipelineAudio {
            ytdlp: ytdlp_cmd.to_string(),
            ytdlp_args: user_args,
            ffmpeg: crate::filtros_ffmpeg::resolver_ffmpeg(),
            query: consulta_o_url.to_string(),
            filtro: opciones.filtro,
            seek: opciones.seek_segundos,
            limiter: opciones.limiter,
            limiter_timeout_ms: opciones.limiter_timeout_ms,
        }));

        let mut call = call_arc.lock().await;
        let pista = Track::new(input).volume(f32::from(opciones.volumen) / 100.0);
        let pista = if opciones.pausada {
            pista.pause()
        } else {
            pista
        };
        let handle = call.play_only(self.observar_pista(guild_id, pista));
        Ok(self.registrar_pista_con_motivo(guild_id, handle, reinicio))
    }

    pub(crate) fn observar_pista(&self, guild_id: u64, mut pista: Track) -> Track {
        pista.events.add_event(
            EventData::new(
                Event::Track(TrackEvent::Play),
                MedidorInicioPista {
                    guild_id,
                    identidad: pista.uuid.as_u128(),
                    solicitado: std::time::Instant::now(),
                },
            ),
            std::time::Duration::ZERO,
        );
        if let Some(oyente) = &*self.oyente_pista.lock().unwrap() {
            let notificador = NotificadorPista {
                guild_id,
                oyente: Arc::clone(oyente),
                identidad: pista.uuid.as_u128(),
            };
            pista.events.add_event(
                EventData::new(Event::Track(TrackEvent::End), notificador),
                std::time::Duration::ZERO,
            );
        }
        pista
    }

    pub(crate) fn registrar_pista(&self, guild_id: u64, handle: TrackHandle) -> TrackHandle {
        self.registrar_pista_con_motivo(guild_id, handle, false)
    }

    /// Notifica "play" en un arranque nuevo y "reinicio" en seek o filtro.
    /// Solo "play" genera registro de historial.
    pub(crate) fn registrar_pista_con_motivo(
        &self,
        guild_id: u64,
        handle: TrackHandle,
        reinicio: bool,
    ) -> TrackHandle {
        self.active_tracks
            .lock()
            .unwrap()
            .insert(guild_id, handle.clone());
        self.notificar_cambio_pista(guild_id, if reinicio { "reinicio" } else { "play" });
        handle
    }
}
