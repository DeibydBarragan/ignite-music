#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotivoDesconexion {
    InactividadVacio,
    InactividadFin,
}

impl MotivoDesconexion {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InactividadVacio => "inactivity_empty",
            Self::InactividadFin => "inactivity_finished",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstadoVoz {
    pub user_id: String,
    pub channel_id: Option<String>,
    pub es_bot: bool,
}

#[derive(Debug, Clone)]
pub struct ConfiguracionPresencia {
    pub leave_on_empty_ms: u64,
    pub leave_on_finish_ms: u64,
    pub allow_bot_input: bool,
    pub sound_bot_id: Option<String>,
    pub bot_user_id: String,
}

impl Default for ConfiguracionPresencia {
    fn default() -> Self {
        Self {
            leave_on_empty_ms: 60_000,
            leave_on_finish_ms: 120_000,
            allow_bot_input: false,
            sound_bot_id: Some("1558180312050180317".into()),
            bot_user_id: String::new(),
        }
    }
}

pub fn contar_personas(
    estados: &[EstadoVoz],
    bot_chan: &str,
    cfg: &ConfiguracionPresencia,
) -> usize {
    estados
        .iter()
        .filter(|s| {
            s.channel_id.as_deref() == Some(bot_chan)
                && (cfg.bot_user_id.is_empty() || s.user_id != cfg.bot_user_id)
                && (!s.es_bot
                    || (cfg.allow_bot_input && cfg.sound_bot_id.as_deref() == Some(&s.user_id)))
        })
        .count()
}

#[async_trait::async_trait]
pub trait AccionesCanalVoz: Send + Sync + 'static {
    async fn pausar_cola(&self, guild_id: &str);
    async fn reanudar_cola(&self, guild_id: &str);
    async fn parar_cola(&self, guild_id: &str);
    async fn tiene_cola_activa(&self, guild_id: &str) -> bool;
    async fn salir_canal(&self, guild_id: &str, motivo: MotivoDesconexion);
    async fn refrescar_panel(&self, guild_id: &str);
    async fn cerrar_panel_vacio(&self, guild_id: &str);
}
