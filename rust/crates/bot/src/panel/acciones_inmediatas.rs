use super::instantanea::InstantaneaCola;
use super::interaccion::ContextoPanel;
use crate::respuesta::{avisar, como_actualizacion, EstadoInteraccion};
use crate::ui::componentes::id;
use crate::ui::panel::vista_reproductor;
use std::collections::HashMap;
use twilight_model::application::interaction::{Interaction, InteractionData};
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use twilight_model::id::marker::GuildMarker;
use twilight_model::id::Id;

pub async fn defer_and_execute(
    cid: &str,
    ctx: &ContextoPanel,
    gstr: &str,
    ei: &EstadoInteraccion,
    lang: &str,
) {
    let resp = InteractionResponse {
        kind: InteractionResponseType::DeferredUpdateMessage,
        data: None,
    };
    let _ = ctx
        .emisor
        .enviar_respuesta(ei.application_id, ei.id, &ei.token, &resp)
        .await;
    let cola_arc = ctx.gestor_colas.obtener_cola(gstr).await;
    let mut g = cola_arc.lock().await;
    let resultado = player::reproduccion::cambiar_pista_con_driver(
        ctx.driver.as_deref(),
        &mut g,
        cid != id::SKIP,
    )
    .await;
    drop(g);
    match resultado {
        Ok(res) if !res.ok => {
            let msg = ctx.idiomas.t(lang, res.clave, &HashMap::new());
            let _ = avisar(&*ctx.emisor, ei, &msg, true).await;
        }
        Err(error) => {
            tracing::error!(guild_id = %gstr, %error, "No se pudo cambiar la pista desde el panel");
            let msg = ctx.idiomas.t(lang, "errors.unknown", &HashMap::new());
            let _ = avisar(&*ctx.emisor, ei, &msg, true).await;
        }
        _ => {}
    }
}

pub async fn ejecutar_accion_inmediata(
    interaction: &Interaction,
    cid: &str,
    ctx: &ContextoPanel,
    gstr: &str,
    gid: Id<GuildMarker>,
    lang: &str,
    ei: &EstadoInteraccion,
) {
    let cola_arc = ctx.gestor_colas.obtener_cola(gstr).await;
    let mut cola = cola_arc.lock().await;
    let mut error_control = None;
    let res = match cid {
        id::PAUSE => {
            match player::reproduccion::alternar_pausa_con_driver(ctx.driver.as_deref(), &mut cola)
                .await
            {
                Ok(resultado) => resultado,
                Err(error) => {
                    error_control = Some(error.to_string());
                    player::ResultadoAccion::no("errors.unknown")
                }
            }
        }
        id::LOOP => {
            let sig = cola.modo_repeticion.siguiente();
            player::acciones::fijar_bucle(&mut cola, Some(sig))
        }
        id::AUTOPLAY => player::acciones::alternar_autoplay(&mut cola),
        id::SHUFFLE => player::acciones::mezclar(&mut cola),
        id::FILTER => {
            let val = extraer_valor_seleccion(interaction).unwrap_or_else(|| "none".to_string());
            let bin = medios::resolve_binary(&ctx.ytdlp_settings);
            match player::reproduccion::fijar_filtro_con_driver(
                ctx.driver.as_deref(),
                &mut cola,
                &val,
                &bin,
                ctx.ytdlp_args.clone(),
                &ctx.ytdlp_settings,
            )
            .await
            {
                Ok(resultado) => resultado,
                Err(error) => {
                    error_control = Some(error.to_string());
                    player::ResultadoAccion::no("errors.unknown")
                }
            }
        }
        _ => return,
    };
    if let Some(error) = error_control {
        tracing::error!(guild_id = %gstr, %error, "No se pudo cambiar el filtro activo");
        let msg = ctx.idiomas.t(lang, "errors.unknown", &HashMap::new());
        let _ = avisar(&*ctx.emisor, ei, &msg, true).await;
        return;
    }
    if !res.ok {
        let msg = ctx.idiomas.t(lang, res.clave, &HashMap::new());
        let _ = avisar(&*ctx.emisor, ei, &msg, true).await;
        return;
    }
    if let Some(snap) = InstantaneaCola::desde_cola(&cola) {
        let payload = como_actualizacion(vista_reproductor(&ctx.idiomas, lang, &snap));
        let update_resp = InteractionResponse {
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
            .enviar_respuesta(ei.application_id, ei.id, &ei.token, &update_resp)
            .await;
        if let Some(msg) = &interaction.message {
            ctx.gestor_paneles
                .adoptar_panel(gid, msg.channel_id, msg.id, lang)
                .await;
        }
    }
}

fn extraer_valor_seleccion(interaction: &Interaction) -> Option<String> {
    match &interaction.data {
        Some(InteractionData::MessageComponent(mc)) => mc.values.first().cloned(),
        _ => None,
    }
}
