use super::formas::*;
use std::collections::HashMap;
use twilight_cache_inmemory::InMemoryCache;
use twilight_model::channel::ChannelType;
use twilight_model::id::marker::{GuildMarker, UserMarker};
use twilight_model::id::Id;
use twilight_model::user::User;

pub fn obtener_canales_cache(
    cache: &InMemoryCache,
    bot_user_id: u64,
    guild_id: &str,
) -> Result<Vec<CanalVozFrontend>, String> {
    let gid = guild_id
        .parse::<Id<GuildMarker>>()
        .map_err(|_| "Guild not found")?;
    let Some(channel_ids) = cache.guild_channels(gid) else {
        return if cache.guild(gid).is_some() {
            Ok(Vec::new())
        } else {
            Err("Guild not found".into())
        };
    };
    let ids: Vec<_> = channel_ids.iter().copied().collect();
    drop(channel_ids);
    let mut canales: Vec<_> = ids
        .into_iter()
        .filter_map(|id| {
            let ch = cache.channel(id)?;
            matches!(
                ch.kind,
                ChannelType::GuildVoice | ChannelType::GuildStageVoice
            )
            .then(|| {
                (
                    ch.position.unwrap_or_default(),
                    ch.parent_id,
                    ch.id,
                    ch.name.clone().unwrap_or_default(),
                    ch.bitrate.unwrap_or(64000),
                    ch.user_limit.unwrap_or(0),
                )
            })
        })
        .collect();
    canales.sort_unstable_by_key(|(posicion, _, id, ..)| (*posicion, *id));
    let mut posicion_por_categoria = HashMap::new();
    let mut ordenados = Vec::with_capacity(canales.len());
    for (_, categoria, id, name, bitrate, user_limit) in canales {
        let posicion = posicion_por_categoria.entry(categoria).or_insert(0usize);
        let ids: Vec<_> = cache
            .voice_channel_states(id)
            .into_iter()
            .flatten()
            .map(|estado| estado.user_id())
            .collect();
        let bot_present = ids.iter().any(|uid| uid.get() == bot_user_id);
        let mut ids = ids;
        ids.sort_unstable();
        let mut user_count = 0;
        let mut members = Vec::new();
        for uid in ids {
            let Some(user) = cache
                .user(uid)
                .filter(|user| !user.bot && uid.get() != bot_user_id)
            else {
                continue;
            };
            user_count += 1;
            if members.len() == 25 {
                continue;
            }
            members.push(MiembroCanalFrontend {
                id: uid.to_string(),
                username: user.name.clone(),
                avatar: Some(avatar_usuario(&user)),
                bot: false,
            });
        }
        ordenados.push((
            *posicion,
            id,
            CanalVozFrontend {
                id: id.to_string(),
                name,
                bitrate,
                user_limit,
                user_count,
                bot_present,
                members,
            },
        ));
        *posicion += 1;
    }
    ordenados.sort_unstable_by_key(|(posicion, id, _)| (*posicion, *id));
    Ok(ordenados.into_iter().map(|(_, _, canal)| canal).collect())
}

fn extension_avatar(hash: &str) -> &'static str {
    if hash.starts_with("a_") {
        "gif"
    } else {
        "webp"
    }
}

fn avatar_usuario(user: &User) -> String {
    if let Some(hash) = user.avatar {
        let hash = hash.to_string();
        return format!(
            "https://cdn.discordapp.com/avatars/{}/{hash}.{}",
            user.id,
            extension_avatar(&hash)
        );
    }
    let indice = if user.discriminator == 0 {
        (user.id.get() >> 22) % 6
    } else {
        u64::from(user.discriminator % 5)
    };
    format!("https://cdn.discordapp.com/embed/avatars/{indice}.png")
}

pub(super) fn datos_solicitante(
    cache: &InMemoryCache,
    guild_id: &str,
    user_id: &str,
) -> Option<UsuarioSolicitante> {
    let guild = guild_id.parse().ok()?;
    let uid = user_id.parse::<Id<UserMarker>>().ok()?;
    let user = cache.user(uid)?;
    let miembro = cache.member(guild, uid);
    let username = miembro
        .as_ref()
        .and_then(|m| m.nick())
        .or(user.global_name.as_deref())
        .unwrap_or(&user.name)
        .to_owned();
    let avatar = miembro.and_then(|m| m.avatar()).map_or_else(
        || avatar_usuario(&user),
        |hash| {
            let hash = hash.to_string();
            format!(
                "https://cdn.discordapp.com/guilds/{guild}/users/{uid}/avatars/{hash}.{}",
                extension_avatar(&hash)
            )
        },
    );
    Some(UsuarioSolicitante {
        id: user_id.to_owned(),
        username,
        avatar: Some(avatar),
    })
}

pub fn obtener_estado_voz_cache(
    cache: &twilight_cache_inmemory::InMemoryCache,
    bot_user_id: u64,
    guild_id: &str,
    user_id: Option<&str>,
) -> Result<EstadoVozRespuesta, String> {
    let gid = guild_id
        .parse::<Id<GuildMarker>>()
        .map_err(|_| "guild_not_found")?;
    if cache.guild(gid).is_none() {
        return Err("guild_not_found".into());
    }
    let bot_vs =
        Id::<UserMarker>::new_checked(bot_user_id).and_then(|id| cache.voice_state(id, gid));
    let bot_status = bot_vs.map(|vs| {
        let cid = vs.channel_id();
        let name = cache
            .channel(cid)
            .and_then(|c| c.name.clone())
            .unwrap_or_default();
        BotVoiceStatus {
            id: cid.to_string(),
            name,
            deaf: vs.deaf() || vs.self_deaf(),
            self_deaf: vs.self_deaf(),
            server_deaf: vs.deaf(),
        }
    });

    let user_status = user_id
        .and_then(|uid_str| uid_str.parse::<Id<UserMarker>>().ok())
        .and_then(|uid| {
            let vs = cache.voice_state(uid, gid)?;
            let cid = vs.channel_id();
            let name = cache
                .channel(cid)
                .and_then(|c| c.name.clone())
                .unwrap_or_default();
            Some(UserVoiceStatus {
                id: cid.to_string(),
                name,
            })
        });

    let can_control = bot_status
        .as_ref()
        .zip(user_status.as_ref())
        .is_some_and(|(bot, user)| bot.id == user.id);

    Ok(EstadoVozRespuesta {
        bot: bot_status,
        user: user_status,
        can_control,
    })
}
