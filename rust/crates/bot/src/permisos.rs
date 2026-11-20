//! Guards y permisos para control de música y canal de panel (Spec S-23).

use player::Cola;
use twilight_cache_inmemory::InMemoryCache;
use twilight_model::channel::ChannelType;
use twilight_model::guild::Permissions;
use twilight_model::id::marker::{ChannelMarker, GuildMarker, UserMarker};
use twilight_model::id::Id;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcionesRevisarVoz {
    pub requiere_cola: bool,
    pub unirse: bool,
}

impl Default for OpcionesRevisarVoz {
    fn default() -> Self {
        Self {
            requiere_cola: true,
            unirse: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoCanalVoz {
    pub id: Id<ChannelMarker>,
    pub lleno: bool,
}

#[derive(Debug)]
pub struct ResultadoVoz<'a> {
    pub canal_id: Id<ChannelMarker>,
    pub cola: Option<&'a Cola>,
}

/// Decide si un usuario puede controlar la música (Spec S-23).
///
/// Evalúa las siguientes reglas en orden y devuelve el primer error:
/// 1. `commands.play.voice_channel_required`: el usuario no está en voz.
/// 2. `commands.play.same_channel_required`: el bot está en voz en otro canal.
/// 3. `commands.nowplaying.no_track`: `requiere_cola` activo y no hay cola o está vacía.
/// 4. `commands.play.no_permissions`: `unirse` activo, bot no está en voz y le faltan permisos.
/// 5. `ui.voice_full`: `unirse` activo, bot no está en voz, canal lleno y bot sin `MoveMembers`.
pub fn revisar_voz<'a>(
    canal_usuario: Option<&InfoCanalVoz>,
    canal_bot: Option<Id<ChannelMarker>>,
    cola: Option<&'a Cola>,
    permisos_bot: Option<Permissions>,
    opciones: OpcionesRevisarVoz,
) -> Result<ResultadoVoz<'a>, &'static str> {
    // 1. Usuario debe estar en voz
    let usuario = match canal_usuario {
        Some(c) => c,
        None => return Err("commands.play.voice_channel_required"),
    };

    // 2. Si el bot ya está en voz, debe ser el mismo canal
    if let Some(bot_cid) = canal_bot {
        if bot_cid != usuario.id {
            return Err("commands.play.same_channel_required");
        }
    }

    // 3. Cola requerida y no vacía
    if opciones.requiere_cola {
        match cola {
            Some(q) if !q.canciones.is_empty() => {}
            _ => return Err("commands.nowplaying.no_track"),
        }
    }

    // 4 y 5. Solo aplican si `unirse` está activo y el bot no está en voz
    if opciones.unirse && canal_bot.is_none() {
        let perms = permisos_bot.unwrap_or(Permissions::empty());
        let requeridos = Permissions::VIEW_CHANNEL | Permissions::CONNECT | Permissions::SPEAK;
        if !perms.contains(requeridos) {
            return Err("commands.play.no_permissions");
        }
        if usuario.lleno && !perms.contains(Permissions::MOVE_MEMBERS) {
            return Err("ui.voice_full");
        }
    }

    Ok(ResultadoVoz {
        canal_id: usuario.id,
        cola,
    })
}

/// Determina si un tipo de canal soporta mensajes de texto según DisTube (Spec S-23).
pub fn es_tipo_canal_con_texto(tipo: ChannelType) -> bool {
    matches!(
        tipo,
        ChannelType::GuildText
            | ChannelType::GuildAnnouncement
            | ChannelType::AnnouncementThread
            | ChannelType::PublicThread
            | ChannelType::PrivateThread
            | ChannelType::GuildVoice
            | ChannelType::GuildStageVoice
    )
}

/// Comprueba si el bot tiene permisos para enviar el panel al canal indicado (Spec S-23).
pub fn es_canal_panel_valido(tipo: ChannelType, permisos: Option<Permissions>) -> bool {
    if !es_tipo_canal_con_texto(tipo) {
        return false;
    }
    let perms = match permisos {
        Some(p) => p,
        None => return false,
    };
    if !perms.contains(Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES) {
        return false;
    }
    if tipo.is_thread() && !perms.contains(Permissions::SEND_MESSAGES_IN_THREADS) {
        return false;
    }
    true
}

/// Devuelve el canal donde se puede enviar el panel, o `None` si no es válido o faltan permisos.
pub fn canal_de_panel(
    channel_id: Id<ChannelMarker>,
    tipo: ChannelType,
    permisos: Option<Permissions>,
) -> Option<Id<ChannelMarker>> {
    if es_canal_panel_valido(tipo, permisos) {
        Some(channel_id)
    } else {
        None
    }
}

/// Obtiene el canal de voz actual de un usuario desde la caché en memoria.
pub fn canal_voz_usuario_desde_cache(
    cache: &InMemoryCache,
    guild_id: Id<GuildMarker>,
    user_id: Id<UserMarker>,
) -> Option<Id<ChannelMarker>> {
    cache
        .voice_state(user_id, guild_id)
        .map(|vs| vs.channel_id())
}

/// Calcula los permisos del bot en un canal determinado usando la caché.
pub fn calcular_permisos_canal_cache(
    cache: &InMemoryCache,
    bot_id: Id<UserMarker>,
    channel_id: Id<ChannelMarker>,
) -> Option<Permissions> {
    cache.permissions().in_channel(bot_id, channel_id).ok()
}
