//! Cliente twilight y despacho de eventos (Spec S-21).

pub use crate::cliente_trabajos::ejecutar_bucle_eventos;
use crate::cliente_trabajos::{TrabajosGateway, MAX_TAREAS_GATEWAY};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{ConfigBuilder, Event, Intents, Shard, ShardId};
use twilight_model::application::interaction::Interaction;
use twilight_model::gateway::payload::incoming::Ready;
use twilight_model::gateway::payload::outgoing::update_presence::UpdatePresencePayload;
use twilight_model::gateway::presence::{Activity, ActivityType, Status};

#[cfg(test)]
mod pruebas;

pub const BOT_INTENTS: Intents =
    Intents::from_bits_truncate(Intents::GUILDS.bits() | Intents::GUILD_VOICE_STATES.bits());

pub fn crear_presencia_arranque(status_texto: &str) -> UpdatePresencePayload {
    let actividad = Activity {
        application_id: None,
        assets: None,
        buttons: Vec::new(),
        created_at: None,
        details: None,
        emoji: None,
        flags: None,
        id: None,
        instance: None,
        name: status_texto.to_string(),
        party: None,
        secrets: None,
        state: None,
        timestamps: None,
        kind: ActivityType::Listening,
        url: None,
    };
    UpdatePresencePayload {
        activities: vec![actividad],
        afk: false,
        since: None,
        status: Status::Online,
    }
}

pub fn crear_cache() -> InMemoryCache {
    InMemoryCache::builder()
        .resource_types(
            ResourceType::GUILD
                | ResourceType::CHANNEL
                | ResourceType::MEMBER
                | ResourceType::ROLE
                | ResourceType::VOICE_STATE,
        )
        .build()
}

pub fn crear_shard(token: &str, status: &str) -> (Shard, twilight_gateway::MessageSender) {
    let presencia = crear_presencia_arranque(status);
    let config = ConfigBuilder::new(token.to_string(), BOT_INTENTS)
        .presence(presencia)
        .build();
    let shard = Shard::with_config(ShardId::ONE, config);
    let sender = shard.sender();
    (shard, sender)
}

pub fn limpiar_cache_antigua(root: &Path) {
    let dir = root.join("audio_cache");
    if dir.exists() {
        match std::fs::remove_dir_all(&dir) {
            Ok(_) => tracing::info!("Removed the old audio_cache folder (no longer used)."),
            Err(e) => tracing::warn!("Could not remove the old audio_cache folder: {}", e),
        }
    }
}

pub async fn verificar_ytdlp(binary_path: &str) -> Result<String, String> {
    medios::ytdlp::version(binary_path).await
}

pub fn verificar_variables_sharding_obsoletas() {
    for var in &["SHARDS", "SHARDING_MANAGER", "SHARDING_MANAGER_MODE"] {
        if std::env::var_os(var).is_some() {
            tracing::warn!(
                "La variable de entorno {} ya no se utiliza en Rust (un solo shard).",
                var
            );
        }
    }
}

#[async_trait::async_trait]
pub trait HooksReady: Send + Sync {
    async fn al_conectar(&self, tag: &str, servidores: usize);
    async fn cargar_comandos(&self);
    async fn restaurar_sesiones(&self);
    async fn iniciar_servicios(&self);
}

pub async fn procesar_ready<H: HooksReady>(
    ready_hecho: &AtomicBool,
    contador_ready: &AtomicUsize,
    ready: &Ready,
    hooks: &H,
) -> bool {
    contador_ready.fetch_add(1, Ordering::SeqCst);
    if ready_hecho
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        iniciar_ready(ready, hooks).await;
        true
    } else {
        tracing::debug!("Evento Ready adicional recibido; omitiendo pasos iniciales.");
        false
    }
}

async fn iniciar_ready<H: HooksReady>(ready: &Ready, hooks: &H) {
    let tag = format!("{}#{:04}", ready.user.name, ready.user.discriminator);
    let servidores = ready.guilds.len();
    tracing::info!("Conectado como {} · {} servidores", tag, servidores);
    hooks.al_conectar(&tag, servidores).await;
    hooks.cargar_comandos().await;
    hooks.restaurar_sesiones().await;
    hooks.iniciar_servicios().await;
}

#[async_trait::async_trait]
pub trait RouterInteraccion: Send + Sync {
    async fn despachar(&self, interaction: Interaction);
}

pub async fn despachar_interaccion<R: RouterInteraccion>(
    interaction: Interaction,
    router: &R,
) -> bool {
    if interaction.guild_id.is_none() {
        tracing::debug!("Interacción ignorada por carecer de guild_id (mensaje directo)");
        return false;
    }
    router.despachar(interaction).await;
    true
}

pub struct DespachadorEventos<H: HooksReady, R: RouterInteraccion> {
    pub cache: Arc<InMemoryCache>,
    pub ready_hecho: Arc<AtomicBool>,
    pub contador_ready: Arc<AtomicUsize>,
    pub hooks_ready: Arc<H>,
    pub router: Arc<R>,
    pub driver: Option<Arc<player::SongbirdDriver>>,
    pub gestor_presencia: Option<Arc<player::GestorPresencia>>,
    pub acciones_voz: Option<Arc<crate::acciones_voz::AccionesVozBot>>,
    pub filtro_voz: Option<Arc<voice::SpeakerFilter>>,
    pub latencia: Arc<AtomicU64>,
}

impl<H: HooksReady + 'static, R: RouterInteraccion + 'static> DespachadorEventos<H, R> {
    pub(crate) async fn procesar_evento(
        &self,
        event: Event,
        trabajos: &mut TrabajosGateway,
    ) -> Result<(), String> {
        self.cache.update(&event);
        match event {
            Event::Ready(ready) => {
                self.contador_ready.fetch_add(1, Ordering::SeqCst);
                if !self.ready_hecho.swap(true, Ordering::SeqCst) {
                    let hooks = self.hooks_ready.clone();
                    trabajos
                        .tareas
                        .spawn(async move { iniciar_ready(&ready, hooks.as_ref()).await });
                }
            }
            Event::InteractionCreate(interaction) => {
                if trabajos.tareas.len() < MAX_TAREAS_GATEWAY {
                    let router = self.router.clone();
                    trabajos.tareas.spawn(async move {
                        despachar_interaccion(interaction.0, router.as_ref()).await;
                    });
                } else {
                    tracing::warn!("Límite de interacciones concurrentes alcanzado");
                }
            }
            Event::GuildCreate(payload) => {
                use twilight_model::gateway::payload::incoming::GuildCreate;
                if let GuildCreate::Available(guild) = payload.as_ref() {
                    for vs in &guild.voice_states {
                        self.encolar_presencia(trabajos, guild.id, vs)?;
                    }
                }
            }
            Event::VoiceStateUpdate(vsu) => {
                if let Some(drv) = &self.driver {
                    drv.process_gateway_event(&Event::VoiceStateUpdate(vsu.clone()))
                        .await;
                }
                if let Some(gid) = vsu.0.guild_id {
                    self.encolar_presencia(trabajos, gid, &vsu.0)?;
                }
            }
            Event::VoiceServerUpdate(vsu) => {
                if let Some(drv) = &self.driver {
                    drv.process_gateway_event(&Event::VoiceServerUpdate(vsu))
                        .await;
                }
            }
            Event::Resumed => {
                tracing::info!("Conexión de gateway reanudada (Resumed)");
            }
            _ => {}
        }
        Ok(())
    }

    fn encolar_presencia(
        &self,
        trabajos: &TrabajosGateway,
        guild: twilight_model::id::Id<twilight_model::id::marker::GuildMarker>,
        estado: &twilight_model::voice::VoiceState,
    ) -> Result<(), String> {
        if let (Some(filtro), Some(member)) = (&self.filtro_voz, &estado.member) {
            filtro.set_known_user(estado.user_id.get(), member.user.bot);
        }
        if let Some(tx) = &trabajos.presencia {
            let estado = player::EstadoVoz {
                user_id: estado.user_id.to_string(),
                channel_id: estado.channel_id.map(|c| c.to_string()),
                es_bot: estado.member.as_ref().is_some_and(|m| m.user.bot),
            };
            tx.try_send((guild.to_string(), estado))
                .map_err(|e| format!("No se pudo entregar el estado de voz a presencia: {e}"))?;
        }
        Ok(())
    }
}
