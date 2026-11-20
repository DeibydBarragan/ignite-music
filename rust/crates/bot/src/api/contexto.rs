//! Contexto y estado compartido para los manejadores de la API (Spec S-27).

mod defecto;
pub use defecto::ProveedorApiDefecto;

use super::almacen_voz::{AlmacenAjustesVoz, AlmacenLogsVoz};
pub use super::dev_mode::GestorDevMode;
use super::formas::*;
use nucleo::Config;
use player::GestorColas;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait ProveedorApi: Send + Sync {
    fn obtener_info_canal_voz(&self, _guild_id: &str, _channel_id: &str) -> Option<CanalVozInfo> {
        None
    }

    async fn obtener_servidores(&self) -> Vec<ServidorFrontend> {
        Vec::new()
    }
    async fn obtener_canales(&self, _guild_id: &str) -> Result<Vec<CanalVozFrontend>, String> {
        Ok(Vec::new())
    }
    async fn estado_voz(&self, _g: &str, _u: Option<&str>) -> Result<EstadoVozRespuesta, String> {
        Ok(EstadoVozRespuesta {
            bot: None,
            user: None,
            can_control: false,
        })
    }
    async fn buscar(
        &self,
        _q: &str,
        _l: usize,
        _guild_id: Option<&str>,
    ) -> Result<Vec<PistaFrontend>, String> {
        Ok(Vec::new())
    }
    async fn sugerir(&self, _q: &str) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    async fn buscar_playlists(
        &self,
        _q: &str,
        _l: usize,
        _guild_id: Option<&str>,
    ) -> Result<Vec<PlaylistFrontend>, String> {
        Ok(Vec::new())
    }
    async fn top_songs(
        &self,
        _g: &str,
        _t: &str,
        _l: usize,
    ) -> Result<Vec<crate::datos::CancionRanking>, String> {
        Ok(Vec::new())
    }
    async fn recomendaciones(
        &self,
        _g: &str,
        _force: bool,
    ) -> Result<RecomendacionRespuesta, String> {
        Ok(RecomendacionRespuesta::default())
    }
    /// Posición y estado de reproducción en vivo del track actual, si existe.
    ///
    /// `None` cuando no se puede obtener (sin driver, sin track activo o error).
    async fn obtener_posicion_en_vivo(&self, _g: &str) -> Option<(u64, bool)> {
        None
    }
    async fn resolver_playlist(
        &self,
        _g: &str,
        _url: &str,
    ) -> Result<ResultadoPlaylistFrontend, String> {
        Err("Not implemented".to_string())
    }
    async fn unirse_voz(
        &self,
        _guild_id: &str,
        channel_id: Option<&str>,
        _user_id: Option<&str>,
    ) -> Result<CanalVozInfo, (u16, &'static str, String)> {
        match channel_id {
            Some(cid) => Ok(CanalVozInfo {
                id: cid.to_string(),
                name: "Voice Channel".to_string(),
            }),
            None => Err((
                400,
                "no_voice_channel",
                "¡No se encontró ningún canal de voz disponible!".to_string(),
            )),
        }
    }
    async fn salir_voz(&self, _guild_id: &str) -> Result<(), String> {
        Ok(())
    }
    async fn verificar_servidor(&self, guild_id: &str) -> Result<(), (u16, &'static str, String)> {
        let svs = self.obtener_servidores().await;
        if !svs.is_empty() && !svs.iter().any(|s| s.id == guild_id) {
            return Err((404, "guild_not_found", "Guild not found".to_string()));
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    async fn reproducir(
        &self,
        _guild_id: &str,
        _query: &str,
        _voice_channel_id: Option<&str>,
        _caller_id: Option<&str>,
        _next: bool,
        _skip: bool,
        _dev_mode: bool,
    ) -> Result<ResultadoReproducirApi, (u16, &'static str, String)> {
        Ok(ResultadoReproducirApi {
            success: true,
            started: true,
            position: 0,
            song: None,
            playlist: None,
        })
    }
    async fn controlar(
        &self,
        _guild_id: &str,
        _action: &str,
        _value: Option<&serde_json::Value>,
    ) -> Result<(bool, String), (u16, &'static str, String)> {
        Ok((true, "OK".to_string()))
    }
}

#[derive(Clone)]
pub struct EstadoApiContexto {
    pub config: Arc<Config>,
    pub gestor_colas: Arc<GestorColas>,
    pub dev_mode: Arc<GestorDevMode>,
    pub ajustes_voz: Arc<AlmacenAjustesVoz>,
    pub logs_voz: Arc<AlmacenLogsVoz>,
    pub sse: Arc<super::sse::HubSse>,
    pub proveedor: Arc<dyn ProveedorApi + Send + Sync>,
}

impl EstadoApiContexto {
    pub fn nuevo(config: Arc<Config>, gestor_colas: Arc<GestorColas>) -> Self {
        Self::nuevo_con_sse(config, gestor_colas, super::sse::HubSse::por_defecto())
    }

    pub fn nuevo_con_sse(
        config: Arc<Config>,
        gestor_colas: Arc<GestorColas>,
        sse: Arc<super::sse::HubSse>,
    ) -> Self {
        let ruta_dev = config.data_dir.join("dev_users.json");
        let sb_client =
            crate::datos::ClienteSupabaseRest::desde_config(&config.supabase).map(Arc::new);
        let gestor_voz = Arc::new(crate::datos::GestorAjustesVoz::nuevo(
            sb_client,
            config.voice.enabled,
            config.voice.default_wake_word.clone(),
            config.voice.tts_feedback_enabled,
        ));
        Self {
            config,
            gestor_colas: gestor_colas.clone(),
            dev_mode: Arc::new(GestorDevMode::nuevo(ruta_dev)),
            ajustes_voz: Arc::new(AlmacenAjustesVoz::con_gestor(gestor_voz)),
            logs_voz: Arc::new(AlmacenLogsVoz::nuevo()),
            sse,
            proveedor: Arc::new(ProveedorApiDefecto(Some(gestor_colas))),
        }
    }

    pub fn con_proveedor(mut self, proveedor: Arc<dyn ProveedorApi + Send + Sync>) -> Self {
        self.proveedor = proveedor;
        self
    }

    pub fn con_logs_voz(mut self, logs_voz: Arc<AlmacenLogsVoz>) -> Self {
        self.logs_voz = logs_voz;
        self
    }
}
