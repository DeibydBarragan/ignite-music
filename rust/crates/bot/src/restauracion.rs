//! Proveedor de restauración de sesiones para Twilight/Songbird (Spec S-19 / S-21).

use player::{ProveedorRestauracion, SongbirdDriver};
use std::sync::Arc;
use std::time::Duration;
use twilight_cache_inmemory::InMemoryCache;
use twilight_model::channel::ChannelType;
use twilight_model::id::marker::ChannelMarker;
use twilight_model::id::Id;

/// Tiempo máximo que se espera a que Discord llene la caché antes de restaurar.
const ESPERA_CACHE: Duration = Duration::from_secs(30);

pub struct ProveedorRestauracionReal {
    pub cache: Arc<InMemoryCache>,
    pub driver: Option<Arc<SongbirdDriver>>,
    pub ytdlp_settings: medios::ytdlp_args::YtDlpSettings,
    pub ytdlp_args: Vec<String>,
}

impl ProveedorRestauracionReal {
    pub fn nuevo(
        cache: Arc<InMemoryCache>,
        driver: Option<Arc<SongbirdDriver>>,
        ytdlp_settings: medios::ytdlp_args::YtDlpSettings,
        ytdlp_args: Vec<String>,
    ) -> Self {
        Self {
            cache,
            driver,
            ytdlp_settings,
            ytdlp_args,
        }
    }
}

#[async_trait::async_trait]
impl ProveedorRestauracion for ProveedorRestauracionReal {
    fn tiene_servidor(&self, guild_id: &str) -> bool {
        guild_id
            .parse::<u64>()
            .ok()
            .and_then(|id| self.cache.guild(Id::new(id)))
            .is_some()
    }

    async fn esperar_cache(&self, guild_ids: &[String]) {
        let limite = tokio::time::Instant::now() + ESPERA_CACHE;
        while !guild_ids.iter().all(|g| self.tiene_servidor(g)) {
            if tokio::time::Instant::now() >= limite {
                tracing::warn!("La caché no tiene todos los servidores guardados tras esperar");
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    async fn canal_voz_valido(&self, _guild_id: &str, canal_id: &str) -> bool {
        canal_id
            .parse::<u64>()
            .ok()
            .and_then(|id| self.cache.channel(Id::new(id)))
            .map(|c| c.kind == ChannelType::GuildVoice || c.kind == ChannelType::GuildStageVoice)
            .unwrap_or(false)
    }

    async fn canal_texto_valido(&self, _guild_id: &str, canal_id: &str) -> bool {
        canal_id
            .parse::<u64>()
            .ok()
            .and_then(|id| self.cache.channel(Id::new(id)))
            .map(|c| c.kind == ChannelType::GuildText)
            .unwrap_or(false)
    }

    async fn contar_oyentes(&self, _guild_id: &str, canal_id: &str) -> usize {
        let Ok(cid_num) = canal_id.parse::<u64>() else {
            return 0;
        };
        let target_cid = Id::<ChannelMarker>::new(cid_num);

        if let Some(states) = self.cache.voice_channel_states(target_cid) {
            return states
                .filter(|vs| {
                    self.cache
                        .user(vs.user_id())
                        .map(|u| !u.bot)
                        .unwrap_or(true)
                })
                .count();
        }
        0
    }

    async fn arrancar_reproduccion(
        &self,
        cola: &mut player::Cola,
        canal_voz_id: &str,
        seek_segundos: Option<u64>,
    ) -> Result<(), String> {
        let drv = self.driver.as_ref().ok_or("no hay driver de voz")?;
        let gid: u64 = cola
            .guild_id
            .parse()
            .map_err(|e| format!("ID de servidor inválido: {e}"))?;
        let vid: u64 = canal_voz_id
            .parse()
            .map_err(|e| format!("ID de canal de voz inválido: {e}"))?;
        drv.join(gid, vid)
            .await
            .map_err(|e| format!("Error conectando Songbird: {e}"))?;

        let bin = medios::resolve_binary(&self.ytdlp_settings);
        let iniciada = player::reproduccion::iniciar_pista_desde_con_settings(
            drv,
            cola,
            &bin,
            self.ytdlp_args.clone(),
            seek_segundos,
            &self.ytdlp_settings,
        )
        .await
        .map_err(|e| format!("Error iniciando la pista: {e}"))?;
        if iniciada {
            Ok(())
        } else {
            Err("la sesión no tiene pista reproducible".into())
        }
    }

    async fn salir_de_voz(&self, guild_id: &str) {
        let (Some(drv), Ok(gid)) = (&self.driver, guild_id.parse::<u64>()) else {
            return;
        };
        if let Err(error) = drv.leave(gid).await {
            tracing::warn!(guild_id = gid, %error, "No se pudo salir del canal tras fallar la restauración");
        }
    }
}
