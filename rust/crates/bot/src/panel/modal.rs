use super::instantanea::InstantaneaCola;
use super::interaccion::ContextoPanel;
use crate::permisos::{
    canal_voz_usuario_desde_cache, revisar_voz, InfoCanalVoz, OpcionesRevisarVoz,
};
use crate::respuesta::{avisar, como_actualizacion, EstadoInteraccion};
use crate::ui::componentes::id;
use crate::ui::panel::vista_reproductor;
use nucleo::idioma::Idiomas;
use std::collections::HashMap;
use twilight_model::application::interaction::modal::ModalInteractionComponent;
use twilight_model::application::interaction::{Interaction, InteractionData};
use twilight_model::channel::message::component::{
    ActionRow, Component, TextInput, TextInputStyle,
};
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use twilight_model::id::marker::ChannelMarker;
use twilight_model::id::Id;

#[allow(deprecated)]
pub fn crear_modal_volumen(idiomas: &Idiomas, lang: &str, volumen: u8) -> InteractionResponse {
    let empty = HashMap::new();
    let tit: String = idiomas
        .t(lang, "buttonhandler.set_volume_title", &empty)
        .chars()
        .take(45)
        .collect();
    let lbl: String = idiomas
        .t(lang, "buttonhandler.volume_label", &empty)
        .chars()
        .take(45)
        .collect();

    let input = Component::TextInput(TextInput {
        custom_id: id::VOLUME_INPUT.to_string(),
        id: None,
        label: Some(lbl),
        max_length: Some(3),
        min_length: Some(1),
        placeholder: None,
        required: Some(true),
        style: TextInputStyle::Short,
        value: Some(volumen.to_string()),
    });

    InteractionResponse {
        kind: InteractionResponseType::Modal,
        data: Some(InteractionResponseData {
            custom_id: Some(id::VOLUME_MODAL.to_string()),
            title: Some(tit),
            components: Some(vec![Component::ActionRow(ActionRow {
                id: None,
                components: vec![input],
            })]),
            ..Default::default()
        }),
    }
}

pub async fn manejar_modal_volumen(interaction: &Interaction, ctx: &ContextoPanel, lang: &str) {
    let Some(guild_id) = interaction.guild_id else {
        return;
    };
    let guild_str = guild_id.to_string();
    let cola_arc = ctx.gestor_colas.obtener_cola(&guild_str).await;
    let mut cola = cola_arc.lock().await;
    let ei = EstadoInteraccion::desde_interaccion(interaction);

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

    let valor_str = extraer_campo_modal(interaction, id::VOLUME_INPUT).unwrap_or_default();
    let valor_limpio = valor_str.trim();
    if !valor_limpio.chars().all(|c| c.is_ascii_digit()) || valor_limpio.is_empty() {
        let msg = ctx
            .idiomas
            .t(lang, "volume.invalid_number", &HashMap::new());
        let _ = avisar(&*ctx.emisor, &ei, &msg, true).await;
        return;
    }
    let vol: u8 = valor_limpio.parse().unwrap_or(255);
    let res = match player::reproduccion::fijar_volumen_con_driver(
        ctx.driver.as_deref(),
        &mut cola,
        vol as i64,
        100,
    )
    .await
    {
        Ok(res) => res,
        Err(error) => {
            tracing::error!(guild_id = %guild_id, %error, "No se pudo aplicar volumen desde el panel");
            let msg = ctx.idiomas.t(lang, "errors.unknown", &HashMap::new());
            let _ = avisar(&*ctx.emisor, &ei, &msg, true).await;
            return;
        }
    };
    if !res.ok {
        let msg = ctx.idiomas.t(lang, res.clave, &HashMap::new());
        let _ = avisar(&*ctx.emisor, &ei, &msg, true).await;
        return;
    }
    let es_panel_actual = match interaction.message.as_ref().map(|m| m.id) {
        Some(mid) => {
            ctx.gestor_paneles
                .es_panel_actual(guild_id, mid, lang)
                .await
        }
        None => false,
    };

    if es_panel_actual {
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
                    .adoptar_panel(guild_id, msg.channel_id, msg.id, lang)
                    .await;
            }
            return;
        }
    }
    let msg = ctx.idiomas.t(lang, res.clave, &HashMap::new());
    let _ = avisar(&*ctx.emisor, &ei, &msg, true).await;
}

fn buscar_en_componente(comp: &ModalInteractionComponent, custom_id: &str) -> Option<String> {
    match comp {
        ModalInteractionComponent::TextInput(ti) if ti.custom_id == custom_id => {
            Some(ti.value.clone())
        }
        ModalInteractionComponent::ActionRow(ar) => {
            for child in &ar.components {
                if let Some(val) = buscar_en_componente(child, custom_id) {
                    return Some(val);
                }
            }
            None
        }
        ModalInteractionComponent::Label(l) => buscar_en_componente(&l.component, custom_id),
        _ => None,
    }
}

fn extraer_campo_modal(interaction: &Interaction, custom_id: &str) -> Option<String> {
    if let Some(InteractionData::ModalSubmit(m)) = &interaction.data {
        for comp in &m.components {
            if let Some(val) = buscar_en_componente(comp, custom_id) {
                return Some(val);
            }
        }
    }
    None
}
