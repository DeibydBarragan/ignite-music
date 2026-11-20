use crate::panel::interaccion::ContextoPanel;
use crate::respuesta::{EstadoInteraccion, PayloadRespuesta};
use crate::ui::componentes::{aviso, contenedor, fila, id, texto, vista_base, DEFAULT_COLOR};
use crate::ui::interaccion::responder_update;
use nucleo::formato::truncar;
use nucleo::idioma::{IdiomaInfo, Idiomas};
use nucleo::idioma_servidor::set_guild_language;
use std::collections::HashMap;
use twilight_model::application::interaction::{Interaction, InteractionData};
use twilight_model::channel::message::component::{
    Component, SelectMenu, SelectMenuOption, SelectMenuType,
};
use twilight_model::guild::Permissions;
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use twilight_model::id::marker::GuildMarker;
use twilight_model::id::Id;

pub fn vista_idioma(
    idiomas: &Idiomas,
    lang: &str,
    languages: &[IdiomaInfo],
    current: &str,
) -> PayloadRespuesta {
    let empty_vars = HashMap::new();
    let current_name = languages
        .iter()
        .find(|l| l.code == current)
        .map(|l| l.name.as_str())
        .unwrap_or(current);

    let title_line = idiomas.t(lang, "commands.language.title", &empty_vars);
    let cur_line = idiomas.t(lang, "commands.language.current", &empty_vars);

    let mut componentes = Vec::new();
    componentes.push(texto(format!(
        "### {title_line}\n{cur_line}: **{current_name}**"
    )));

    let placeholder = truncar(
        &idiomas.t(lang, "commands.language.select", &empty_vars),
        150,
    );

    let opciones: Vec<SelectMenuOption> = languages
        .iter()
        .take(25)
        .map(|l| SelectMenuOption {
            default: l.code == current,
            description: None,
            emoji: None,
            label: l.name.clone(),
            value: l.code.clone(),
        })
        .collect();

    componentes.push(fila(vec![Component::SelectMenu(SelectMenu {
        channel_types: None,
        custom_id: id::LANGUAGE.to_string(),
        default_values: None,
        disabled: false,
        id: None,
        kind: SelectMenuType::Text,
        max_values: None,
        min_values: None,
        options: Some(opciones),
        placeholder: Some(placeholder),
        required: None,
    })]));

    let c = contenedor(Some(DEFAULT_COLOR), componentes);
    vista_base(vec![c], true)
}

pub async fn manejar_seleccion_idioma(
    interaction: &Interaction,
    ctx: &ContextoPanel,
    guild_id: Id<GuildMarker>,
    lang: &str,
    ei: &EstadoInteraccion,
) {
    let perms = interaction
        .member
        .as_ref()
        .and_then(|m| m.permissions)
        .unwrap_or_else(Permissions::empty);

    if !perms.contains(Permissions::MANAGE_GUILD) {
        let msg = ctx.idiomas.t(
            lang,
            "commands.language.permission_required_button",
            &HashMap::new(),
        );
        let payload = aviso(&msg, true, Some(DEFAULT_COLOR));
        let resp = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(InteractionResponseData {
                components: Some(payload.components),
                flags: payload.flags,
                ..Default::default()
            }),
        };
        let _ = ctx
            .emisor
            .enviar_respuesta(ei.application_id, ei.id, &ei.token, &resp)
            .await;
        return;
    }

    let code = match &interaction.data {
        Some(InteractionData::MessageComponent(mc)) => mc.values.first().cloned(),
        _ => None,
    };
    let Some(code) = code else {
        return;
    };

    let data_dir = std::path::PathBuf::from("data");
    let _ = set_guild_language(&ctx.idiomas, &data_dir, &guild_id.to_string(), &code).await;

    let lang_name = ctx
        .idiomas
        .list()
        .iter()
        .find(|l| l.code == code)
        .map(|l| l.name.clone())
        .unwrap_or_else(|| code.clone());

    let mut changed_vars = HashMap::new();
    changed_vars.insert("language".to_string(), lang_name);
    let msg = ctx
        .idiomas
        .t(&code, "commands.language.changed_desc", &changed_vars);
    responder_update(&*ctx.emisor, ei, aviso(&msg, true, Some(DEFAULT_COLOR))).await;

    let cola_arc = ctx.gestor_colas.obtener_cola(&guild_id.to_string()).await;
    let cola = cola_arc.lock().await;
    if let Some(snap) = crate::panel::InstantaneaCola::desde_cola(&cola) {
        drop(cola);
        ctx.gestor_paneles
            .refrescar_panel(guild_id, snap, false, &code)
            .await;
    }
}
