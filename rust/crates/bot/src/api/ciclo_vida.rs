//! Orquestador del ciclo de vida de la API, túnel, anunciador y SSE (Specs S-26..S-30).

use super::anuncio::Anunciador;
use super::contexto::EstadoApiContexto;
use super::proveedor_real::ProveedorApiReal;
use super::sse::HubSse;
use super::tunel::GestorTunel;
use super::ServidorApiHandle;
use nucleo::Config;
use std::sync::Arc;
use std::time::{Duration, Instant};
use twilight_cache_inmemory::InMemoryCache;

const PLAZO_CIERRE_SERVIDOR_API: Duration = Duration::from_secs(2);

pub struct ServiciosApiBot {
    pub servidor: Option<ServidorApiHandle>,
    pub tunel: Option<Arc<GestorTunel>>,
    pub anunciador: Option<Arc<Anunciador>>,
    pub hub_sse: Arc<HubSse>,
    pub almacen_logs_voz: Arc<voice::AlmacenRegistroVoz>,
    pub ajustes_voz: Arc<crate::api::almacen_voz::AlmacenAjustesVoz>,
    pub proveedor_real: Arc<ProveedorApiReal>,
    pub contexto: EstadoApiContexto,
    tarea_posicion: Option<tokio::task::JoinHandle<()>>,
}

impl ServiciosApiBot {
    #[allow(clippy::too_many_arguments)]
    pub async fn iniciar(
        config: Arc<Config>,
        cache: Arc<InMemoryCache>,
        http: Option<Arc<twilight_http::Client>>,
        driver: Option<Arc<player::SongbirdDriver>>,
        gestor_colas: Arc<player::GestorColas>,
        exportador_csv: Option<Arc<voice::ExportadorCsvVoz>>,
        ytdlp_settings: medios::YtDlpSettings,
        controles: super::proveedor_control::ConfiguracionControles,
    ) -> Result<Self, String> {
        let hub_sse = HubSse::por_defecto();
        let almacen_logs_voz = Arc::new(voice::AlmacenRegistroVoz::new(
            100,
            exportador_csv,
            Some(hub_sse.clone()),
        ));

        let sb_client =
            crate::datos::ClienteSupabaseRest::desde_config(&config.supabase).map(Arc::new);
        if let Some(ref sb) = sb_client {
            let res = sb.verify_connection(3000).await;
            if res.ok {
                tracing::info!("Supabase connection verified");
            } else {
                tracing::warn!(
                    "Supabase connection check: {}",
                    res.reason.as_deref().unwrap_or("unknown")
                );
            }
        }
        let hist_dir = config.data_dir.join("history");
        let _ = std::fs::create_dir_all(&hist_dir);
        let exclusiones = Arc::new(crate::datos::ExclusionesHistorial::nuevo(
            hist_dir.join("exclusions.json"),
        ));
        let historial = Arc::new(crate::datos::GestorHistorial::nuevo(
            sb_client.clone(),
            &config.data_dir,
            exclusiones,
        ));
        let gestor_ranking = Some(Arc::new(crate::datos::GestorRanking::nuevo(
            sb_client.clone(),
            historial,
        )));

        let bot_user_id: u64 = config.discord.client_id.parse().unwrap_or(0);
        let ytdlp_bin = medios::resolve_binary(&ytdlp_settings);
        let ytdlp_args = medios::base_args(&ytdlp_settings, None, None);

        let spotify_client =
            if !config.spotify.client_id.is_empty() && !config.spotify.client_secret.is_empty() {
                Some(Arc::new(medios::SpotifyClient::new(
                    config.spotify.client_id.clone(),
                    config.spotify.client_secret.clone(),
                )))
            } else {
                None
            };

        let prov_real = Arc::new(
            ProveedorApiReal::nuevo(
                cache,
                http,
                driver,
                gestor_colas.clone(),
                gestor_ranking,
                bot_user_id,
                ytdlp_settings,
                config.proxies.clone(),
                spotify_client,
                config.bot.max_queue_size as usize,
                config.bot.max_playlist_size as usize,
            )
            .con_controles(controles),
        );
        let logs_voz = Arc::new(crate::api::almacen_voz::AlmacenLogsVoz::con_almacen_real(
            almacen_logs_voz.clone(),
        ));
        let ctx_api =
            EstadoApiContexto::nuevo_con_sse(config.clone(), gestor_colas.clone(), hub_sse.clone())
                .con_proveedor(prov_real.clone())
                .con_logs_voz(logs_voz);
        let ajustes_voz = ctx_api.ajustes_voz.clone();
        // El listener comparte el gestor de dev-mode con POST /play para decidir el registro.
        super::eventos_reproduccion::conectar(
            &prov_real,
            &hub_sse,
            &gestor_colas,
            &ytdlp_bin,
            &ytdlp_args,
            &prov_real.ytdlp_settings,
            &config.proxies,
            prov_real.gestor_ranking.clone(),
            ctx_api.dev_mode.clone(),
            config.dev_user_ids.clone(),
        );

        let servidor =
            crate::api::iniciar_servidor_api_con_contexto(ctx_api.clone(), Instant::now()).await?;

        let tarea_posicion = prov_real.driver.as_ref().map(|driver| {
            let ctx = ctx_api.clone();
            let driver = driver.clone();
            tokio::spawn(super::eventos_reproduccion::sincronizar_posicion(
                ctx, driver,
            ))
        });
        let mut gestor_tunel = None;
        if config.api.enabled && config.api.auto_tunnel {
            let bin = crate::api::tunel::resolver_binario(&config.bin_dir, &config.root);
            let gt = Arc::new(GestorTunel::con_binario(bin));
            gestor_tunel = Some(gt.clone());
            let port = config.api.port;
            let timeout_ms = crate::api::DEFAULT_TUNNEL_TIMEOUT_MS;
            tokio::spawn(async move {
                gt.iniciar(port, Duration::from_millis(timeout_ms)).await;
            });
        }

        let mut anunciador = None;
        if config.api.enabled {
            let sb_dyn: Option<Arc<dyn crate::api::anuncio::ClienteSupabase>> =
                sb_client.map(|c| c as Arc<dyn crate::api::anuncio::ClienteSupabase>);
            let an = Anunciador::nuevo(sb_dyn, gestor_tunel.clone(), config.api.public_url.clone());
            an.iniciar(crate::api::DEFAULT_ANNOUNCE_INTERVAL);
            anunciador = Some(an);
        }

        Ok(Self {
            servidor,
            tunel: gestor_tunel,
            anunciador,
            hub_sse,
            almacen_logs_voz,
            ajustes_voz,
            proveedor_real: prov_real,
            contexto: ctx_api,
            tarea_posicion,
        })
    }

    pub async fn apagar(&mut self) {
        if let Some(tarea) = self.tarea_posicion.take() {
            tarea.abort();
            let _ = tarea.await;
        }
        if let Some(ref an) = self.anunciador {
            an.detener().await;
        }
        if let Some(ref gt) = self.tunel {
            gt.detener().await;
        }
        self.hub_sse.cerrar();
        if let Some(srv) = self.servidor.take() {
            apagar_servidor_acotado(srv).await;
        }
    }
}

/// Espera el cierre del servidor API durante un plazo fijo y después aborta la tarea.
/// Un cliente SSE que no lee puede mantener su conexión abierta; sin plazo, el apagado
/// no llegaría a guardar sesiones. Llamar antes `HubSse::cerrar` para que los streams terminen.
/// shortcut: plazo fijo de 2 s y abort sin cierre ordenado, subir si un cliente legítimo necesita más.
pub async fn apagar_servidor_acotado(srv: ServidorApiHandle) {
    let ServidorApiHandle {
        shutdown_tx,
        mut join_handle,
        ..
    } = srv;
    if let Some(tx) = shutdown_tx {
        let _ = tx.send(());
    }
    if tokio::time::timeout(PLAZO_CIERRE_SERVIDOR_API, &mut join_handle)
        .await
        .is_err()
    {
        tracing::warn!(
            "Servidor API no cerró en {:?}; cortando conexiones",
            PLAZO_CIERRE_SERVIDOR_API
        );
        join_handle.abort();
    }
}
