use crate::comandos::Contexto;
use crate::respuesta::{responder, EmisorRespuesta, EstadoInteraccion};
use crate::ui::busqueda::vista_busqueda;
use crate::ui::cache_tokens::CacheTokensVistas;
use crate::ui::componentes::{aviso, DEFAULT_COLOR};
use medios::errores::error_key;
use medios::modelos::FuentesError;
use medios::youtube_busqueda::buscar;
use medios::ytdlp_args::resolve_binary;
use std::collections::HashMap;
use twilight_model::channel::message::MessageFlags;
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use twilight_model::id::marker::UserMarker;
use twilight_model::id::Id;

#[allow(clippy::too_many_arguments)]
pub async fn ejecutar_search(
    ctx: &Contexto,
    guild_id: &str,
    user_id: Id<UserMarker>,
    lang: &str,
    raw_query: &str,
    cache_tokens: &CacheTokensVistas,
    emisor: &dyn EmisorRespuesta,
    ei: &EstadoInteraccion,
) {
    let query = raw_query.trim();
    if query.is_empty() {
        let payload = aviso(
            "Debes ingresar un término de búsqueda.",
            true,
            Some(DEFAULT_COLOR),
        );
        let _ = responder(emisor, ei, &payload).await;
        return;
    }

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

    let mut estado_diferido = ei.clone();
    estado_diferido.diferida = true;

    let settings = ctx.ytdlp_settings.clone();
    let binario = resolve_binary(&settings);
    let limiter = settings.limiter.clone();
    let search_res = buscar(
        &binario,
        query,
        10,
        Some(guild_id),
        &settings,
        &[],
        &limiter,
    )
    .await;

    match search_res {
        Err(err) => {
            let (code, msg) = match &err {
                FuentesError::UrlNoSoportada => (Some("NOT_SUPPORTED_URL"), None),
                FuentesError::ListaVacia => (Some("EMPTY_PLAYLIST"), None),
                FuentesError::ListaFiltradaVacia => (Some("EMPTY_FILTERED_PLAYLIST"), None),
                FuentesError::NsfwNoPermitido => (Some("NON_NSFW"), None),
                FuentesError::ResultadoVacio => (Some("NO_RESULT"), None),
                FuentesError::SinStreamUrl(s) => (Some("NO_STREAM_URL"), Some(s.as_str())),
                FuentesError::YtDlp(s) => (None, Some(s.as_str())),
                FuentesError::Spotify(_) => (None, None),
            };
            let err_key = error_key(code, msg, msg);
            let msg = ctx.idiomas.t(lang, err_key, &HashMap::new());
            let payload = aviso(&msg, true, Some(DEFAULT_COLOR));
            let _ = responder(emisor, &estado_diferido, &payload).await;
        }
        Ok(results) if results.is_empty() => {
            let msg = ctx
                .idiomas
                .t(lang, "commands.search.no_results", &HashMap::new());
            let payload = aviso(&msg, true, Some(DEFAULT_COLOR));
            let _ = responder(emisor, &estado_diferido, &payload).await;
        }
        Ok(results) => {
            let token = format!("{:012x}", fastrand::u64(..0x1000_0000_0000));
            cache_tokens
                .guardar_busqueda(token.clone(), user_id, results.clone())
                .await;
            let payload = vista_busqueda(&ctx.idiomas, lang, query, &results, &token);
            let _ = responder(emisor, &estado_diferido, &payload).await;
        }
    }
}
