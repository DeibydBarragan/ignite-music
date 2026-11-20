use super::acciones_inmediatas::{defer_and_execute, ejecutar_accion_inmediata};
use super::modal::crear_modal_volumen;
use super::nowplaying::manejar_favorito;
use super::GestorPaneles;
use crate::permisos::{
    canal_voz_usuario_desde_cache, revisar_voz, InfoCanalVoz, OpcionesRevisarVoz,
};
use crate::respuesta::{avisar, EmisorRespuesta, EstadoInteraccion};
use crate::ui::componentes::id;
use crate::ui::panel::vista_terminada;
use nucleo::idioma::Idiomas;
use player::GestorColas;
use std::collections::HashMap;
use std::sync::Arc;
use twilight_cache_inmemory::InMemoryCache;
use twilight_model::application::interaction::Interaction;
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use twilight_model::id::marker::ChannelMarker;
use twilight_model::id::Id;

pub struct ContextoPanel {
    pub gestor_colas: Arc<GestorColas>,
    pub gestor_paneles: Arc<GestorPaneles>,
    pub idiomas: Arc<Idiomas>,
    pub emisor: Arc<dyn EmisorRespuesta>,
    pub cache: Option<Arc<InMemoryCache>>,
    pub supabase_url: Option<String>,
    pub gestor_favoritos: Option<Arc<crate::datos::GestorFavoritos>>,
    pub driver: Option<Arc<player::SongbirdDriver>>,
    pub ytdlp_settings: medios::YtDlpSettings,
    pub ytdlp_args: Vec<String>,
}

pub async fn manejar_interaccion_panel(
    interaction: &Interaction,
    custom_id: &str,
    ctx: &ContextoPanel,
    lang: &str,
) {
    let Some(guild_id) = interaction.guild_id else {
        return;
    };
    let guild_str = guild_id.to_string();
    let cola_arc = ctx.gestor_colas.obtener_cola(&guild_str).await;
    let cola = cola_arc.lock().await;
    let ei = EstadoInteraccion::desde_interaccion(interaction);

    let msg_id = interaction.message.as_ref().map(|m| m.id);
    let es_actual = match msg_id {
        Some(mid) => {
            ctx.gestor_paneles
                .es_panel_actual(guild_id, mid, lang)
                .await
        }
        None => false,
    };

    if !cola.esta_activa() || !es_actual {
        let payload = vista_terminada(
            &ctx.idiomas,
            lang,
            "musicmanager.playback_ended",
            None,
            None,
        );
        let resp = InteractionResponse {
            kind: InteractionResponseType::UpdateMessage,
            data: Some(InteractionResponseData {
                components: Some(payload.components),
                flags: payload.flags,
                allowed_mentions: payload.allowed_mentions,
                ..Default::default()
            }),
        };
        let _ = ctx
            .emisor
            .enviar_respuesta(ei.application_id, ei.id, &ei.token, &resp)
            .await;
        return;
    }

    if custom_id == id::QUEUE {
        let items: Vec<crate::ui::cola::DatosItemCola> = cola
            .canciones
            .iter()
            .map(|s| crate::ui::cola::DatosItemCola {
                cancion: s.cancion.clone(),
                requester_id: s.requester_id.clone(),
                requester_es_bot: false,
            })
            .collect();
        let actual = items.first();
        let proximas = if items.len() > 1 { &items[1..] } else { &[] };
        let payload = crate::ui::cola::vista_cola(&ctx.idiomas, lang, actual, proximas, 0);
        let resp = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(InteractionResponseData {
                components: Some(payload.components),
                flags: payload.flags,
                allowed_mentions: payload.allowed_mentions,
                ..Default::default()
            }),
        };
        let _ = ctx
            .emisor
            .enviar_respuesta(ei.application_id, ei.id, &ei.token, &resp)
            .await;
        return;
    }

    if custom_id == id::LYRICS {
        if let Some(first) = cola.canciones.first() {
            let name = first.cancion.nombre.clone();
            let artist = first.cancion.artista.clone();
            let dur = first.cancion.duracion;
            drop(cola);
            let tokens = crate::ui::cache_tokens::CacheTokensVistas::nuevo();
            crate::comandos::letras::mostrar_letras(
                &*ctx.emisor,
                &ei,
                &ctx.idiomas,
                lang,
                &name,
                Some(&artist),
                dur,
                &tokens,
                None,
            )
            .await;
        } else {
            let msg = ctx
                .idiomas
                .t(lang, "commands.nowplaying.no_track", &HashMap::new());
            let _ = avisar(&*ctx.emisor, &ei, &msg, true).await;
        }
        return;
    }

    if custom_id == id::FAVORITE {
        let user_id = interaction
            .author_id()
            .map(|u| u.to_string())
            .unwrap_or_default();
        manejar_favorito(&cola, ctx, &ei, &user_id).await;
        return;
    }

    let canal_bot: Option<Id<ChannelMarker>> =
        cola.canal_voz.as_deref().and_then(|s| s.parse().ok());
    let usuario_cid = ctx
        .cache
        .as_ref()
        .and_then(|c| {
            interaction
                .author_id()
                .and_then(|uid| canal_voz_usuario_desde_cache(c, guild_id, uid))
        })
        .or(canal_bot);

    let info_usuario = usuario_cid.map(|cid| InfoCanalVoz {
        id: cid,
        lleno: false,
    });
    let opciones = OpcionesRevisarVoz {
        requiere_cola: true,
        unirse: false,
    };
    if let Err(err_key) = revisar_voz(
        info_usuario.as_ref(),
        canal_bot,
        Some(&cola),
        None,
        opciones,
    ) {
        let msg = ctx.idiomas.t(lang, err_key, &HashMap::new());
        let _ = avisar(&*ctx.emisor, &ei, &msg, true).await;
        return;
    }
    drop(cola);

    match custom_id {
        id::PAUSE | id::LOOP | id::AUTOPLAY | id::SHUFFLE | id::FILTER => {
            ejecutar_accion_inmediata(interaction, custom_id, ctx, &guild_str, guild_id, lang, &ei)
                .await;
        }
        id::VOLUME => {
            let cola = cola_arc.lock().await;
            let modal_resp = crear_modal_volumen(&ctx.idiomas, lang, cola.volumen);
            let _ = ctx
                .emisor
                .enviar_respuesta(ei.application_id, ei.id, &ei.token, &modal_resp)
                .await;
        }
        id::SKIP | id::PREVIOUS => {
            defer_and_execute(custom_id, ctx, &guild_str, &ei, lang).await;
        }
        id::STOP => {
            let resp = InteractionResponse {
                kind: InteractionResponseType::DeferredUpdateMessage,
                data: None,
            };
            let _ = ctx
                .emisor
                .enviar_respuesta(ei.application_id, ei.id, &ei.token, &resp)
                .await;
            let mut cola = cola_arc.lock().await;
            if let Err(error) =
                player::reproduccion::parar_con_driver(ctx.driver.as_deref(), &mut cola).await
            {
                tracing::error!(guild_id = %guild_str, %error, "No se pudo detener la música desde el panel");
            }
            drop(cola);
            ctx.gestor_paneles
                .cerrar_panel(guild_id, "musicmanager.playback_ended", None, None, lang)
                .await;
        }
        _ => {}
    }
}
