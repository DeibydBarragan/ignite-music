//! Proveedor real de la API conectado a Discord, reproductor, medios y Supabase (Spec S-27).

use super::contexto::ProveedorApi;
use super::formas::*;
use super::proveedor_real_aux::*;
use medios::modelos::ResultadoResolucion;
use medios::ytdlp_args::YtDlpSettings;
use std::sync::Arc;
use twilight_cache_inmemory::InMemoryCache;
use twilight_model::id::marker::GuildMarker;
use twilight_model::id::Id;

pub struct ProveedorApiReal {
    pub(super) controles: Option<super::proveedor_control::ConfiguracionControles>,
    pub cache: Arc<InMemoryCache>,
    pub http: Option<Arc<twilight_http::Client>>,
    pub driver: Option<Arc<player::SongbirdDriver>>,
    pub gestor_colas: Arc<player::GestorColas>,
    pub gestor_ranking: Option<Arc<crate::datos::GestorRanking>>,
    pub bot_user_id: u64,
    pub ytdlp_settings: YtDlpSettings,
    pub proxies: Vec<String>,
    pub spotify_client: Option<Arc<medios::SpotifyClient>>,
    pub max_queue_size: usize,
    pub max_playlist_size: usize,
    pub http_client: reqwest::Client,
    pub(super) radio: super::proveedor_recomendaciones::Radio,
    /// Inyección para pruebas de POST /play: sustituye la resolución por yt-dlp. None en producción.
    pub(super) prueba_resolucion: Option<Arc<dyn Fn() -> ResultadoResolucion + Send + Sync>>,
    /// Inyección para pruebas de POST /play: omite la unión a voz de Discord. false en producción.
    pub(super) prueba_sin_union: bool,
}

impl ProveedorApiReal {
    #[allow(clippy::too_many_arguments)]
    pub fn nuevo(
        cache: Arc<InMemoryCache>,
        http: Option<Arc<twilight_http::Client>>,
        driver: Option<Arc<player::SongbirdDriver>>,
        gestor_colas: Arc<player::GestorColas>,
        gestor_ranking: Option<Arc<crate::datos::GestorRanking>>,
        bot_user_id: u64,
        ytdlp_settings: YtDlpSettings,
        proxies: Vec<String>,
        spotify_client: Option<Arc<medios::SpotifyClient>>,
        max_queue_size: usize,
        max_playlist_size: usize,
    ) -> Self {
        Self {
            controles: None,
            cache,
            http,
            driver,
            gestor_colas,
            gestor_ranking,
            bot_user_id,
            ytdlp_settings,
            proxies,
            spotify_client,
            max_queue_size,
            max_playlist_size,
            http_client: reqwest::Client::new(),
            radio: super::proveedor_recomendaciones::Radio::nuevo(),
            prueba_resolucion: None,
            prueba_sin_union: false,
        }
    }
}

#[async_trait::async_trait]
impl ProveedorApi for ProveedorApiReal {
    fn obtener_info_canal_voz(&self, guild_id: &str, channel_id: &str) -> Option<CanalVozInfo> {
        let guild_id = Id::<GuildMarker>::new(guild_id.parse().ok()?);
        let channel_id =
            Id::<twilight_model::id::marker::ChannelMarker>::new(channel_id.parse().ok()?);
        if !self.cache.guild_channels(guild_id)?.contains(&channel_id) {
            return None;
        }
        let channel = self.cache.channel(channel_id)?;
        if !matches!(
            channel.kind,
            twilight_model::channel::ChannelType::GuildVoice
                | twilight_model::channel::ChannelType::GuildStageVoice
        ) {
            return None;
        }
        Some(CanalVozInfo {
            id: channel.id.to_string(),
            name: channel.name.clone().unwrap_or_default(),
        })
    }

    async fn obtener_servidores(&self) -> Vec<ServidorFrontend> {
        let mut servidores = Vec::new();
        for guild in self.cache.iter().guilds() {
            servidores.push(ServidorFrontend {
                id: guild.id().to_string(),
                name: guild.name().to_string(),
                icon: guild.icon().map(|i| {
                    let hash = i.to_string();
                    let ext = extension_icono(&hash);
                    format!(
                        "https://cdn.discordapp.com/icons/{}/{}.{}?size=256",
                        guild.id(),
                        hash,
                        ext
                    )
                }),
                member_count: guild.member_count().unwrap_or(0) as usize,
            });
        }
        servidores
    }

    async fn obtener_canales(&self, guild_id: &str) -> Result<Vec<CanalVozFrontend>, String> {
        obtener_canales_cache(&self.cache, self.bot_user_id, guild_id)
    }

    async fn estado_voz(&self, g: &str, u: Option<&str>) -> Result<EstadoVozRespuesta, String> {
        obtener_estado_voz_cache(&self.cache, self.bot_user_id, g, u)
    }

    async fn buscar(
        &self,
        q: &str,
        l: usize,
        guild_id: Option<&str>,
    ) -> Result<Vec<PistaFrontend>, String> {
        buscar_en_medios(q, l, guild_id, &self.ytdlp_settings, &self.proxies).await
    }

    async fn sugerir(&self, q: &str) -> Result<Vec<String>, String> {
        sugerir_consultas(&self.http_client, q).await
    }

    async fn buscar_playlists(
        &self,
        q: &str,
        l: usize,
        guild_id: Option<&str>,
    ) -> Result<Vec<PlaylistFrontend>, String> {
        buscar_listas_en_medios(q, l, guild_id, &self.ytdlp_settings, &self.proxies).await
    }

    async fn obtener_posicion_en_vivo(&self, g: &str) -> Option<(u64, bool)> {
        let gid_u64: u64 = g.parse().ok()?;
        let drv = self.driver.as_ref()?;
        let handle = drv.active_track(gid_u64)?;
        let info = tokio::time::timeout(std::time::Duration::from_secs(1), handle.get_info())
            .await
            .ok()?
            .ok()?;
        let pos = info.position.as_secs();
        let is_playing = matches!(info.playing, songbird::tracks::PlayMode::Play)
            && matches!(info.ready, songbird::tracks::ReadyState::Playable);
        Some((pos, is_playing))
    }

    async fn recomendaciones(
        &self,
        g: &str,
        force: bool,
    ) -> Result<RecomendacionRespuesta, String> {
        super::proveedor_recomendaciones::recomendaciones(self, g, force).await
    }

    async fn top_songs(
        &self,
        g: &str,
        t: &str,
        l: usize,
    ) -> Result<Vec<crate::datos::CancionRanking>, String> {
        if let Some(ref gr) = self.gestor_ranking {
            Ok(gr.get_top_songs(g, t, l).await)
        } else {
            Ok(Vec::new())
        }
    }

    async fn resolver_playlist(
        &self,
        g: &str,
        url: &str,
    ) -> Result<ResultadoPlaylistFrontend, String> {
        resolver_lista_en_medios(self, g, url).await
    }

    async fn unirse_voz(
        &self,
        guild_id: &str,
        channel_id: Option<&str>,
        user_id: Option<&str>,
    ) -> Result<CanalVozInfo, (u16, &'static str, String)> {
        super::proveedor_real_operaciones::unirse_voz(self, guild_id, channel_id, user_id).await
    }

    async fn salir_voz(&self, guild_id: &str) -> Result<(), String> {
        let gid_u64: u64 = guild_id.parse().map_err(|e| format!("{e}"))?;
        if let Some(ref drv) = self.driver {
            drv.leave(gid_u64).await.map_err(|e| format!("{e}"))?;
        }
        Ok(())
    }

    async fn verificar_servidor(&self, guild_id: &str) -> Result<(), (u16, &'static str, String)> {
        let gid = guild_id
            .parse::<Id<GuildMarker>>()
            .map_err(|_| (404, "guild_not_found", "Guild not found".into()))?;
        if self.cache.iter().guilds().next().is_some() && self.cache.guild(gid).is_none() {
            return Err((404, "guild_not_found", "Guild not found".into()));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn reproducir(
        &self,
        guild_id: &str,
        query: &str,
        voice_channel_id: Option<&str>,
        caller_id: Option<&str>,
        next: bool,
        skip: bool,
        dev_mode: bool,
    ) -> Result<ResultadoReproducirApi, (u16, &'static str, String)> {
        super::proveedor_reproduccion::ejecutar_reproduccion_api(
            self,
            guild_id,
            query,
            voice_channel_id,
            caller_id,
            next,
            skip,
            dev_mode,
        )
        .await
    }

    async fn controlar(
        &self,
        guild_id: &str,
        action: &str,
        value: Option<&serde_json::Value>,
    ) -> Result<(bool, String), (u16, &'static str, String)> {
        super::proveedor_control::ejecutar_control_api(self, guild_id, action, value).await
    }
}
