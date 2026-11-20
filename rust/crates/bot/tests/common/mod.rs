//! Helpers compartidos para pruebas de integración de la API (Specs S-26..S-30).

use bot::api::contexto::{EstadoApiContexto, ProveedorApi};
use bot::api::formas::*;
use bot::api::proveedor_real::ProveedorApiReal;
use bot::api::sse::HubSse;
use medios::ytdlp_args::YtDlpSettings;
use nucleo::Config;
use player::cola::ElementoCola;
use player::GestorColas;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use twilight_cache_inmemory::InMemoryCache;

pub const SECRET: &str = "test-secret-0123456789abcdef0123456789abcdef";
pub const GUILD: &str = "123456789012345678";
pub const USER: &str = "345678901234567890";
pub const VOICE_CH: &str = "234567890123456789";

pub struct ProveedorE2e {
    pub real: ProveedorApiReal,
}

#[async_trait::async_trait]
impl ProveedorApi for ProveedorE2e {
    async fn obtener_servidores(&self) -> Vec<ServidorFrontend> {
        self.real.obtener_servidores().await
    }
    async fn obtener_canales(&self, g: &str) -> Result<Vec<CanalVozFrontend>, String> {
        self.real.obtener_canales(g).await
    }
    async fn estado_voz(&self, g: &str, u: Option<&str>) -> Result<EstadoVozRespuesta, String> {
        self.real.estado_voz(g, u).await
    }
    async fn buscar(
        &self,
        q: &str,
        l: usize,
        g: Option<&str>,
    ) -> Result<Vec<PistaFrontend>, String> {
        self.real.buscar(q, l, g).await
    }
    async fn sugerir(&self, q: &str) -> Result<Vec<String>, String> {
        self.real.sugerir(q).await
    }
    async fn buscar_playlists(
        &self,
        q: &str,
        l: usize,
        g: Option<&str>,
    ) -> Result<Vec<PlaylistFrontend>, String> {
        self.real.buscar_playlists(q, l, g).await
    }
    async fn top_songs(
        &self,
        g: &str,
        t: &str,
        l: usize,
    ) -> Result<Vec<bot::datos::CancionRanking>, String> {
        self.real.top_songs(g, t, l).await
    }
    async fn resolver_playlist(
        &self,
        g: &str,
        u: &str,
    ) -> Result<ResultadoPlaylistFrontend, String> {
        self.real.resolver_playlist(g, u).await
    }
    async fn unirse_voz(
        &self,
        g: &str,
        c: Option<&str>,
        u: Option<&str>,
    ) -> Result<CanalVozInfo, (u16, &'static str, String)> {
        self.real.unirse_voz(g, c, u).await
    }
    async fn salir_voz(&self, g: &str) -> Result<(), String> {
        self.real.salir_voz(g).await
    }
    async fn verificar_servidor(&self, g: &str) -> Result<(), (u16, &'static str, String)> {
        self.real.verificar_servidor(g).await
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
        _dev_mode: bool,
    ) -> Result<ResultadoReproducirApi, (u16, &'static str, String)> {
        if query.contains("unconfigured-spotify") {
            return Err((
                400,
                "SPOTIFY_NOT_CONFIGURED",
                "Spotify client not configured".into(),
            ));
        }
        let song = medios::Cancion {
            fuente: "youtube".to_string(),
            id: "dQw4w9WgXcQ".to_string(),
            nombre: "Never Gonna Give You Up".to_string(),
            artista: "Rick Astley".to_string(),
            url: query.to_string(),
            duracion: 213,
            stream_url: Some("http://127.0.0.1:0/stream.opus".to_string()),
            es_directo: false,
            miniatura: Some("https://example.com/thumb.jpg".to_string()),
            autor_url: None,
            edad_restringida: false,
            es_letra: false,
        };
        let mut elem = ElementoCola::nuevo(song, caller_id.map(ToString::to_string));
        elem.requester_name = Some("Listener".into());
        let front = serializar_elemento_cola(&elem, false);
        let cola_arc = self.real.gestor_colas.obtener_cola(guild_id).await;
        let mut cola = cola_arc.lock().await;
        cola.canal_voz = Some(voice_channel_id.unwrap_or(VOICE_CH).to_string());
        let pos = if next { 1 } else { 0 };
        let res_encolar = player::encolar(
            &mut cola,
            vec![elem],
            pos,
            skip,
            self.real.max_queue_size,
            true,
        )
        .map_err(|e| (400, "ENQUEUE_ERROR", e.to_string()))?;
        Ok(ResultadoReproducirApi {
            success: true,
            started: res_encolar.empezada,
            position: res_encolar.posicion,
            song: Some(front),
            playlist: None,
        })
    }
    async fn controlar(
        &self,
        guild_id: &str,
        action: &str,
        value: Option<&serde_json::Value>,
    ) -> Result<(bool, String), (u16, &'static str, String)> {
        self.real.controlar(guild_id, action, value).await
    }
}

pub async fn spawn_e2e_api() -> (String, Arc<GestorColas>, Arc<HubSse>) {
    let mut cfg = Config::from_map(
        std::path::PathBuf::from("."),
        &std::collections::HashMap::new(),
    );
    cfg.api.secret = SECRET.to_string();
    cfg.api.enabled = true;
    cfg.data_dir = std::env::temp_dir().join(format!("ignite_e2e_{}", fastrand::u64(..)));

    let cache = Arc::new(InMemoryCache::new());
    let gestor_colas = Arc::new(GestorColas::nuevo());
    let hub_sse = HubSse::por_defecto();

    let real = ProveedorApiReal::nuevo(
        cache,
        None,
        None,
        gestor_colas.clone(),
        None,
        111111111111111111,
        YtDlpSettings::default(),
        vec!["http://proxy.local:8080".into()],
        None,
        100,
        100,
    );
    let real = real.con_controles(bot::api::proveedor_control::ConfiguracionControles {
        idiomas: Arc::new(
            nucleo::Idiomas::cargar(
                &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../languages"),
                Some("en"),
            )
            .unwrap(),
        ),
        legacy: Arc::default(),
        data_dir: cfg.data_dir.clone(),
        max_volumen: cfg.bot.max_volume as u8,
    });
    let prov = Arc::new(ProveedorE2e { real });
    let ctx =
        EstadoApiContexto::nuevo_con_sse(Arc::new(cfg), gestor_colas.clone(), hub_sse.clone())
            .con_proveedor(prov);

    let app = bot::api::crear_router_con_contexto(Instant::now(), ctx);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });

    (format!("http://127.0.0.1:{port}"), gestor_colas, hub_sse)
}
