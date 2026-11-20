use super::cache_tokens::CacheTokensVistas;
use super::cola::{vista_cola, DatosItemCola};
use super::idioma::manejar_seleccion_idioma;
use super::letras::vista_letras;
use crate::panel::interaccion::ContextoPanel;
use crate::respuesta::{como_actualizacion, EmisorRespuesta, EstadoInteraccion, PayloadRespuesta};
use crate::ui::componentes::{aviso, id, DEFAULT_COLOR};
use std::collections::HashMap;
use twilight_model::application::interaction::{Interaction, InteractionData};
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use twilight_model::id::marker::GuildMarker;
use twilight_model::id::Id;

pub(crate) async fn responder_update(
    emisor: &dyn EmisorRespuesta,
    ei: &EstadoInteraccion,
    payload: PayloadRespuesta,
) {
    let update = como_actualizacion(payload);
    let resp = InteractionResponse {
        kind: InteractionResponseType::UpdateMessage,
        data: Some(InteractionResponseData {
            components: Some(update.components),
            flags: update.flags,
            allowed_mentions: update.allowed_mentions,
            ..Default::default()
        }),
    };
    let _ = emisor
        .enviar_respuesta(ei.application_id, ei.id, &ei.token, &resp)
        .await;
}

pub async fn manejar_interaccion_auxiliar(
    interaction: &Interaction,
    custom_id: &str,
    ctx: &ContextoPanel,
    cache_tokens: &CacheTokensVistas,
    lang: &str,
) -> bool {
    let Some(guild_id) = interaction.guild_id else {
        return false;
    };
    let ei = EstadoInteraccion::desde_interaccion(interaction);

    if custom_id.starts_with(id::QUEUE_PAGE) {
        manejar_pagina_cola(custom_id, ctx, guild_id, lang, &ei).await;
        return true;
    }
    if custom_id.starts_with(id::LYRICS_PAGE) {
        manejar_pagina_letras(custom_id, ctx, cache_tokens, lang, &ei).await;
        return true;
    }
    if custom_id.starts_with(id::SEARCH) {
        manejar_seleccion_busqueda(interaction, custom_id, ctx, cache_tokens, lang, &ei).await;
        return true;
    }
    if custom_id == id::LANGUAGE {
        manejar_seleccion_idioma(interaction, ctx, guild_id, lang, &ei).await;
        return true;
    }
    false
}

async fn manejar_pagina_cola(
    custom_id: &str,
    ctx: &ContextoPanel,
    guild_id: Id<GuildMarker>,
    lang: &str,
    ei: &EstadoInteraccion,
) {
    let guild_str = guild_id.to_string();
    let cola_arc = ctx.gestor_colas.obtener_cola(&guild_str).await;
    let cola = cola_arc.lock().await;

    if cola.canciones.is_empty() {
        let msg = ctx
            .idiomas
            .t(lang, "commands.nowplaying.no_track", &HashMap::new());
        responder_update(&*ctx.emisor, ei, aviso(&msg, true, Some(DEFAULT_COLOR))).await;
        return;
    }

    let page: i64 = custom_id[id::QUEUE_PAGE.len()..].parse().unwrap_or(0);
    let items: Vec<DatosItemCola> = cola
        .canciones
        .iter()
        .map(|s| DatosItemCola {
            cancion: s.cancion.clone(),
            requester_id: s.requester_id.clone(),
            requester_es_bot: false,
        })
        .collect();

    let actual = items.first();
    let proximas = if items.len() > 1 { &items[1..] } else { &[] };
    let payload = vista_cola(&ctx.idiomas, lang, actual, proximas, page);
    responder_update(&*ctx.emisor, ei, payload).await;
}

async fn manejar_pagina_letras(
    custom_id: &str,
    ctx: &ContextoPanel,
    cache_tokens: &CacheTokensVistas,
    lang: &str,
    ei: &EstadoInteraccion,
) {
    let rest = &custom_id[id::LYRICS_PAGE.len()..];
    let mut parts = rest.split(':');
    let token = parts.next().unwrap_or("");
    let page: i64 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);

    let payload = match cache_tokens.obtener_letras(token).await {
        Some(f) => vista_letras(
            &ctx.idiomas,
            lang,
            &f.title,
            Some(&f.artist),
            &f.lyrics,
            Some(&f.source),
            Some(token),
            page,
        ),
        None => {
            let msg = ctx
                .idiomas
                .t(lang, "buttonhandler.lyrics_unavailable", &HashMap::new());
            aviso(&msg, true, Some(DEFAULT_COLOR))
        }
    };
    responder_update(&*ctx.emisor, ei, payload).await;
}

async fn manejar_seleccion_busqueda(
    interaction: &Interaction,
    custom_id: &str,
    ctx: &ContextoPanel,
    cache_tokens: &CacheTokensVistas,
    lang: &str,
    ei: &EstadoInteraccion,
) {
    let token = &custom_id[id::SEARCH.len()..];
    let selected_idx = match &interaction.data {
        Some(InteractionData::MessageComponent(mc)) => {
            mc.values.first().and_then(|v| v.parse::<usize>().ok())
        }
        _ => None,
    };

    let entry = cache_tokens.obtener_busqueda(token).await;
    let author_matches = interaction
        .author_id()
        .map(|uid| entry.as_ref().map(|e| e.0 == uid).unwrap_or(false))
        .unwrap_or(false);

    let valid_song = entry
        .as_ref()
        .and_then(|(_, list)| selected_idx.and_then(|i| list.get(i)));

    if !author_matches || valid_song.is_none() {
        let msg = ctx
            .idiomas
            .t(lang, "buttonhandler.search_expired", &HashMap::new());
        responder_update(&*ctx.emisor, ei, aviso(&msg, true, Some(DEFAULT_COLOR))).await;
        return;
    }

    let song = valid_song.unwrap();
    let resp = InteractionResponse {
        kind: InteractionResponseType::DeferredUpdateMessage,
        data: None,
    };
    let _ = ctx
        .emisor
        .enviar_respuesta(ei.application_id, ei.id, &ei.token, &resp)
        .await;

    if let Some(gid) = interaction.guild_id {
        let cola_arc = ctx.gestor_colas.obtener_cola(&gid.to_string()).await;
        let mut cola = cola_arc.lock().await;
        let req_id = interaction.author_id().map(|id| id.to_string());
        cola.canciones
            .push(player::cola::ElementoCola::nuevo(song.clone(), req_id));
    }
}
