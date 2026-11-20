use crate::comandos::Contexto;
use crate::respuesta::{responder, EmisorRespuesta, EstadoInteraccion};
use crate::ui::cache_tokens::CacheTokensVistas;
use crate::ui::componentes::{aviso, DEFAULT_COLOR};
use crate::ui::letras::vista_letras;
use medios::letras::{buscar_letras, CacheLetras};
use nucleo::idioma::Idiomas;
use std::collections::HashMap;
use twilight_model::channel::message::MessageFlags;
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};

#[allow(clippy::too_many_arguments)]
pub async fn mostrar_letras(
    emisor: &dyn EmisorRespuesta,
    ei: &EstadoInteraccion,
    idiomas: &Idiomas,
    lang: &str,
    target_name: &str,
    target_artist: Option<&str>,
    duration: u64,
    cache_tokens: &CacheTokensVistas,
    cache_letras: Option<&CacheLetras>,
) {
    if !ei.diferida && !ei.respondida {
        let defer_resp = InteractionResponse {
            kind: InteractionResponseType::DeferredChannelMessageWithSource,
            data: Some(InteractionResponseData {
                flags: Some(MessageFlags::EPHEMERAL | MessageFlags::IS_COMPONENTS_V2),
                ..Default::default()
            }),
        };
        let _ = emisor
            .enviar_respuesta(ei.application_id, ei.id, &ei.token, &defer_resp)
            .await;
    }

    let client = reqwest::Client::new();
    let artist_str = target_artist.unwrap_or("");
    let lookup_res = buscar_letras(&client, target_name, artist_str, duration, cache_letras).await;

    let mut estado_diferido = ei.clone();
    estado_diferido.diferida = true;

    match lookup_res {
        Err(_) => {
            let msg = idiomas.t(lang, "buttonhandler.lyrics_error", &HashMap::new());
            let payload = aviso(&msg, true, Some(DEFAULT_COLOR));
            let _ = responder(emisor, &estado_diferido, &payload).await;
        }
        Ok(None) => {
            let no_lyrics = idiomas.t(lang, "buttonhandler.no_lyrics_found", &HashMap::new());
            let msg = format!("{no_lyrics}\n-# {target_name}");
            let payload = aviso(&msg, true, Some(DEFAULT_COLOR));
            let _ = responder(emisor, &estado_diferido, &payload).await;
        }
        Ok(Some(found)) => {
            let token = format!("{:012x}", fastrand::u64(..0x1000_0000_0000));
            cache_tokens
                .guardar_letras(token.clone(), found.clone())
                .await;
            let payload = vista_letras(
                idiomas,
                lang,
                &found.title,
                Some(&found.artist),
                &found.lyrics,
                Some(&found.source),
                Some(&token),
                0,
            );
            let _ = responder(emisor, &estado_diferido, &payload).await;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn ejecutar_lyrics(
    ctx: &Contexto,
    guild_id: &str,
    lang: &str,
    query_opt: Option<&str>,
    cache_tokens: &CacheTokensVistas,
    cache_letras: Option<&CacheLetras>,
    emisor: &dyn EmisorRespuesta,
    ei: &EstadoInteraccion,
) {
    if let Some(query) = query_opt.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        mostrar_letras(
            emisor,
            ei,
            &ctx.idiomas,
            lang,
            query,
            None,
            0,
            cache_tokens,
            cache_letras,
        )
        .await;
        return;
    }

    let cola_arc = ctx.gestor_colas.obtener_cola(guild_id).await;
    let cola = cola_arc.lock().await;
    if let Some(first) = cola.canciones.first() {
        let name = first.cancion.nombre.clone();
        let artist = first.cancion.artista.clone();
        let dur = first.cancion.duracion;
        drop(cola);
        mostrar_letras(
            emisor,
            ei,
            &ctx.idiomas,
            lang,
            &name,
            Some(&artist),
            dur,
            cache_tokens,
            cache_letras,
        )
        .await;
    } else {
        let msg = ctx
            .idiomas
            .t(lang, "commands.nowplaying.no_track", &HashMap::new());
        let payload = aviso(&msg, true, Some(DEFAULT_COLOR));
        let _ = responder(emisor, ei, &payload).await;
    }
}
