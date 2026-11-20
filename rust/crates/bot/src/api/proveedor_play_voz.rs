use super::proveedor_real::ProveedorApiReal;
use twilight_model::channel::ChannelType;
use twilight_model::guild::Permissions;
use twilight_model::id::{
    marker::{ChannelMarker, GuildMarker, UserMarker},
    Id,
};

type ErrorVoz = (u16, &'static str, String);

pub(super) async fn seleccionar_canal(
    proveedor: &ProveedorApiReal,
    guild: &str,
    solicitado: Option<&str>,
    usuario: Option<&str>,
    dev: bool,
) -> Result<(Id<GuildMarker>, Id<ChannelMarker>), ErrorVoz> {
    let cache = &proveedor.cache;
    let gid = guild
        .parse::<Id<GuildMarker>>()
        .map_err(|_| (400, "INVALID_SNOWFLAKE", "Invalid guild ID".into()))?;
    if cache.guild(gid).is_none() {
        return Err((404, "guild_not_found", "Guild not found".into()));
    }
    let bot = Id::<UserMarker>::new_checked(proveedor.bot_user_id);
    let canal_de = |uid| {
        cache
            .voice_state(uid, gid)
            .map(|estado| estado.channel_id())
    };
    let canal_bot = bot.and_then(canal_de);
    let canal_usuario = usuario.and_then(|uid| uid.parse().ok()).and_then(canal_de);
    let valido = |cid| {
        cache.channel(cid).is_some_and(|c| {
            c.guild_id == Some(gid)
                && matches!(
                    c.kind,
                    ChannelType::GuildVoice | ChannelType::GuildStageVoice
                )
        })
    };
    let canal_solicitado = solicitado
        .and_then(|cid| cid.parse().ok())
        .filter(|cid| valido(*cid));
    let sin_canal = || {
        (
            400,
            "NOT_IN_VOICE",
            "¡Debes estar en un canal de voz en Discord para reproducir música!".into(),
        )
    };

    // Node toma el canal del solicitante antes que la preferencia enviada por el dashboard.
    let elegido = canal_usuario.or(canal_solicitado);
    if let Some(canal) = elegido {
        if let Some(actual) =
            canal_bot.filter(|actual| *actual != canal && (!dev || usuario.is_none()))
        {
            let nombre = cache
                .channel(actual)
                .and_then(|c| c.name.clone())
                .unwrap_or_default();
            return Err((
                400,
                "WRONG_VOICE_CHANNEL",
                format!("¡Debes estar en el mismo canal de voz que el bot ({nombre})!"),
            ));
        }
        return Ok((gid, canal));
    }
    if usuario.is_some() && solicitado.filter(|cid| !cid.is_empty()).is_none() && !dev {
        return Err(sin_canal());
    }
    if dev {
        if let Some(canal) = canal_bot {
            return Ok((gid, canal));
        }
    }
    if proveedor.gestor_colas.tiene_cola(guild).await {
        let cola = proveedor.gestor_colas.obtener_cola(guild).await;
        let cola = cola.lock().await;
        if let Some(cid) = cola
            .canal_voz
            .as_deref()
            .and_then(|id| id.parse().ok())
            .filter(|cid| valido(*cid))
        {
            return Ok((gid, cid));
        }
    }
    if !dev {
        return Err(sin_canal());
    }

    let canales =
        super::proveedor_real_aux_voz::obtener_canales_cache(cache, proveedor.bot_user_id, guild)
            .map_err(|_| sin_canal())?;
    let necesarios = Permissions::VIEW_CHANNEL | Permissions::CONNECT | Permissions::SPEAK;
    canales
        .into_iter()
        .filter(|canal| {
            let Some(cid) = canal.id.parse::<Id<ChannelMarker>>().ok() else {
                return false;
            };
            bot.is_none_or(|uid| {
                cache
                    .permissions()
                    .in_channel(uid, cid)
                    .is_ok_and(|p| p.contains(necesarios))
            })
        })
        .min_by_key(|canal| canal.user_count == 0)
        .and_then(|canal| canal.id.parse().ok())
        .map(|cid| (gid, cid))
        .ok_or_else(sin_canal)
}
