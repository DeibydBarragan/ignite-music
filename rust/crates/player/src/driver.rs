//! Songbird voice driver for Discord audio playback and connection management (Spec S-16).

use crate::driver_error::to_nz;
pub use crate::driver_error::DriverError;
use songbird::{
    driver::{DecodeConfig, DecodeMode},
    events::{Event, EventContext, EventHandler},
    shards::TwilightMap,
    tracks::{PlayMode, Track, TrackHandle},
    Call, Config, Songbird,
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::debug;
use twilight_gateway::MessageSender;
use twilight_model::id::{marker::UserMarker, Id};

pub type OyentePista = Arc<dyn Fn(u64, &'static str, Option<u128>) + Send + Sync>;

pub(crate) struct NotificadorPista {
    pub(crate) guild_id: u64,
    pub(crate) oyente: OyentePista,
    pub(crate) identidad: u128,
}

pub(crate) use crate::driver_inicio::MedidorInicioPista;

#[async_trait::async_trait]
impl EventHandler for NotificadorPista {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(estados) = ctx {
            if let Some((estado, _)) = estados.first() {
                let evento = if matches!(estado.playing, PlayMode::Errored(_)) {
                    "error"
                } else {
                    "end"
                };
                (self.oyente)(self.guild_id, evento, Some(self.identidad));
            }
        }
        None
    }
}

pub struct SongbirdDriver {
    pub(crate) songbird: Arc<Songbird>,
    pub(crate) active_tracks: Mutex<HashMap<u64, TrackHandle>>,
    pub(crate) oyente_pista: Mutex<Option<OyentePista>>,
    pub(crate) conexiones: crate::driver_conexion::Conexiones,
}

impl SongbirdDriver {
    pub fn new(songbird: Arc<Songbird>) -> Self {
        Self {
            songbird,
            active_tracks: Mutex::new(HashMap::new()),
            oyente_pista: Mutex::new(None),
            conexiones: Default::default(),
        }
    }

    pub fn create_songbird(
        bot_id: Id<UserMarker>,
        senders: HashMap<u32, MessageSender>,
    ) -> Arc<Songbird> {
        let map = Arc::new(TwilightMap::new(senders));
        let config = Config::default().decode_mode(DecodeMode::Decode(DecodeConfig::default()));
        Arc::new(Songbird::twilight_from_config(map, bot_id, config))
    }

    pub fn songbird(&self) -> &Arc<Songbird> {
        &self.songbird
    }

    pub fn fijar_oyente_pista(&self, oyente: OyentePista) {
        *self.oyente_pista.lock().unwrap() = Some(oyente);
    }

    pub fn notificar_cambio_pista(&self, guild_id: u64, evento: &'static str) {
        if let Some(ref o) = *self.oyente_pista.lock().unwrap() {
            (o)(guild_id, evento, None);
        }
    }

    pub fn get_or_insert(
        &self,
        guild_id: u64,
    ) -> Result<Arc<tokio::sync::Mutex<Call>>, DriverError> {
        Ok(self.songbird.get_or_insert(to_nz(guild_id)?))
    }

    pub async fn process_gateway_event(&self, event: &twilight_gateway::Event) {
        self.songbird.process(event).await;
    }

    pub fn is_connected(&self, guild_id: u64) -> bool {
        if let Ok(nz) = to_nz(guild_id) {
            self.songbird.get(nz).is_some()
        } else {
            false
        }
    }

    pub async fn play_file<P: AsRef<Path>>(
        &self,
        guild_id: u64,
        path: P,
    ) -> Result<TrackHandle, DriverError> {
        let path_buf = path.as_ref().to_path_buf();
        if !path_buf.exists() {
            return Err(DriverError::Input(format!(
                "Audio file does not exist: {}",
                path_buf.display()
            )));
        }

        let g_nz = to_nz(guild_id)?;
        let call_arc = self
            .songbird
            .get(g_nz)
            .ok_or(DriverError::NotInChannel(guild_id))?;

        debug!(guild_id, path = %path_buf.display(), "Playing local audio file");
        let src = songbird::input::File::new(path_buf);
        let mut call = call_arc.lock().await;
        let pista = self.observar_pista(guild_id, Track::new(src.into()));
        let handle = call.play(pista);

        Ok(self.registrar_pista(guild_id, handle))
    }

    pub async fn pause(&self, guild_id: u64) -> Result<(), DriverError> {
        let track = {
            let tracks = self.active_tracks.lock().unwrap();
            tracks
                .get(&guild_id)
                .cloned()
                .ok_or(DriverError::NoTrack(guild_id))?
        };
        let res = track
            .pause()
            .map_err(|e| DriverError::TrackControl(format!("{:?}", e)));
        self.notificar_cambio_pista(guild_id, "pause");
        res
    }

    pub async fn resume(&self, guild_id: u64) -> Result<(), DriverError> {
        let track = {
            let tracks = self.active_tracks.lock().unwrap();
            tracks
                .get(&guild_id)
                .cloned()
                .ok_or(DriverError::NoTrack(guild_id))?
        };
        let res = track
            .play()
            .map_err(|e| DriverError::TrackControl(format!("{:?}", e)));
        self.notificar_cambio_pista(guild_id, "resume");
        res
    }

    pub async fn stop(&self, guild_id: u64) -> Result<(), DriverError> {
        if let Some(track) = self.active_tracks.lock().unwrap().remove(&guild_id) {
            let _ = track.stop();
        }
        if let Ok(g_nz) = to_nz(guild_id) {
            if let Some(call_arc) = self.songbird.get(g_nz) {
                let mut call = call_arc.lock().await;
                call.stop();
            }
        }
        self.notificar_cambio_pista(guild_id, "stop");
        Ok(())
    }

    pub async fn detener_y_avanzar(&self, guild_id: u64) -> Result<(), DriverError> {
        self.stop(guild_id).await?;
        self.notificar_cambio_pista(guild_id, "advance");
        Ok(())
    }

    /// Consume un evento final una sola vez y solo para el handle vigente.
    pub fn aceptar_fin(&self, guild_id: u64, identidad: u128) -> bool {
        let mut pistas = self.active_tracks.lock().unwrap();
        if pistas
            .get(&guild_id)
            .is_some_and(|p| p.uuid().as_u128() == identidad)
        {
            pistas.remove(&guild_id);
            true
        } else {
            false
        }
    }

    pub async fn set_volume(&self, guild_id: u64, volume: f32) -> Result<(), DriverError> {
        let track = {
            let tracks = self.active_tracks.lock().unwrap();
            tracks
                .get(&guild_id)
                .cloned()
                .ok_or(DriverError::NoTrack(guild_id))?
        };
        let res = track
            .set_volume(volume)
            .map_err(|e| DriverError::TrackControl(format!("{:?}", e)));
        self.notificar_cambio_pista(guild_id, "volume");
        res
    }

    pub async fn set_deaf(&self, guild_id: u64, deaf: bool) -> Result<(), DriverError> {
        let g_nz = to_nz(guild_id)?;
        if let Some(call_arc) = self.songbird.get(g_nz) {
            let mut call = call_arc.lock().await;
            call.deafen(deaf)
                .await
                .map_err(|e| DriverError::TrackControl(format!("{:?}", e)))?;
        }
        self.notificar_cambio_pista(guild_id, if deaf { "deafen" } else { "undeafen" });
        Ok(())
    }

    /// Snapshot de los handles de pista activos por guild.
    ///
    /// Devuelve clones (cheap) para poder hacer `get_info()` fuera del lock del mapa.
    pub fn active_track_handles(&self) -> Vec<(u64, TrackHandle)> {
        self.active_tracks
            .lock()
            .unwrap()
            .iter()
            .map(|(g, h)| (*g, h.clone()))
            .collect()
    }

    /// Handle de la pista activa del guild, si existe.
    pub fn active_track(&self, guild_id: u64) -> Option<TrackHandle> {
        self.active_tracks.lock().unwrap().get(&guild_id).cloned()
    }
}
