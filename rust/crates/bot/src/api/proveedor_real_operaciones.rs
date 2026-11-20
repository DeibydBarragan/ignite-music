use super::formas::*;
use super::proveedor_real::ProveedorApiReal;
use twilight_model::id::marker::{ChannelMarker, GuildMarker, UserMarker};
use twilight_model::id::Id;

pub(super) async fn unirse_voz(
    proveedor: &ProveedorApiReal,
    guild_id: &str,
    channel_id: Option<&str>,
    user_id: Option<&str>,
) -> Result<CanalVozInfo, (u16, &'static str, String)> {
    let gid_u64: u64 = guild_id
        .parse()
        .map_err(|e| (400, "invalid_guild", format!("{e}")))?;
    let gid = Id::<GuildMarker>::new(gid_u64);

    let canales = proveedor.cache.guild_channels(gid);
    let valido = |id: u64| {
        let cid = Id::<ChannelMarker>::new(id);
        canales.as_ref().is_some_and(|ids| ids.contains(&cid))
            && proveedor.cache.channel(cid).is_some_and(|c| {
                matches!(
                    c.kind,
                    twilight_model::channel::ChannelType::GuildVoice
                        | twilight_model::channel::ChannelType::GuildStageVoice
                )
            })
    };
    let solicitado = channel_id
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|id| *id != 0 && valido(*id));
    let usuario = user_id
        .and_then(|u| u.parse::<u64>().ok())
        .filter(|u| *u != 0)
        .and_then(|u| {
            proveedor
                .cache
                .voice_state(Id::<UserMarker>::new(u), gid)
                .map(|vs| vs.channel_id().get())
        })
        .filter(|id| valido(*id));
    let respaldo = canales
        .as_ref()
        .and_then(|ids| ids.iter().map(|id| id.get()).filter(|id| valido(*id)).min());
    let con_personas = proveedor.cache.guild_voice_states(gid).and_then(|users| {
        users
            .iter()
            .filter_map(|user| {
                let humano = proveedor.cache.user(*user).is_some_and(|u| !u.bot);
                humano
                    .then(|| proveedor.cache.voice_state(*user, gid))
                    .flatten()
                    .map(|state| state.channel_id().get())
            })
            .filter(|id| valido(*id))
            .min()
    });
    let target_cid = solicitado.or(usuario).or(con_personas).or(respaldo);

    let cid_u64 = target_cid.ok_or((
        400,
        "no_voice_channel",
        "¡No se encontró ningún canal de voz disponible!".into(),
    ))?;
    conectar_canal_voz(proveedor, gid_u64, cid_u64).await
}

pub(super) async fn conectar_canal_voz(
    proveedor: &ProveedorApiReal,
    gid_u64: u64,
    cid_u64: u64,
) -> Result<CanalVozInfo, (u16, &'static str, String)> {
    let cid = Id::<ChannelMarker>::new(cid_u64);
    let ch_name = proveedor
        .cache
        .channel(cid)
        .and_then(|ch| {
            (ch.guild_id.is_some_and(|gid| gid.get() == gid_u64)
                && matches!(
                    ch.kind,
                    twilight_model::channel::ChannelType::GuildVoice
                        | twilight_model::channel::ChannelType::GuildStageVoice
                ))
            .then(|| ch.name.clone().unwrap_or_else(|| "Voice Channel".into()))
        })
        .ok_or((
            404,
            "voice_channel_not_found",
            "Voice channel not found".into(),
        ))?;
    if proveedor.bot_user_id != 0 {
        if let Ok(permisos) = proveedor
            .cache
            .permissions()
            .in_channel(Id::<UserMarker>::new(proveedor.bot_user_id), cid)
        {
            if !permisos.contains(twilight_model::guild::Permissions::CONNECT) {
                return Err((
                    403,
                    "voice_permission_denied",
                    "No tengo permisos para unirme al canal de voz.".into(),
                ));
            }
        }
    }
    if let Some(ref drv) = proveedor.driver {
        drv.join(gid_u64, cid_u64)
            .await
            .map_err(|e| (500, "voice_join_failed", e.to_string()))?;
    }
    Ok(CanalVozInfo {
        id: cid_u64.to_string(),
        name: ch_name,
    })
}
