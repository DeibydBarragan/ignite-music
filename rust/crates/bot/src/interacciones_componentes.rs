//! Despacho de componentes, modales y autocompletado (Specs S-24, S-25).

use crate::interacciones::{extraer_consulta_autocompletado, RouterInteraccionesReal};
use crate::panel::interaccion::ContextoPanel;
use crate::respuesta::EmisorRespuestaReal;
use std::sync::Arc;
use twilight_http::Client as HttpClient;
use twilight_model::application::interaction::application_command::CommandData;
use twilight_model::application::interaction::Interaction;
use twilight_model::id::marker::GuildMarker;
use twilight_model::id::Id;

pub async fn despachar_componente(
    router: &RouterInteraccionesReal,
    interaction: &Interaction,
    custom_id: &str,
    guild_id: Id<GuildMarker>,
    http: &Arc<HttpClient>,
) {
    let lang = nucleo::idioma_servidor::guild_language(
        &router.ctx.idiomas,
        &router.ctx.data_dir,
        &router.ctx.legacy_languages,
        Some(&guild_id.to_string()),
        None,
    );

    let ctx_panel = ContextoPanel {
        gestor_colas: router.ctx.gestor_colas.clone(),
        gestor_paneles: router.gestor_paneles.clone(),
        idiomas: router.ctx.idiomas.clone(),
        emisor: Arc::new(EmisorRespuestaReal::nuevo(http.clone())),
        cache: router.cache.clone(),
        supabase_url: None,
        gestor_favoritos: router.ctx.gestor_favoritos.clone(),
        driver: router.driver.clone(),
        ytdlp_settings: router.ctx.ytdlp_settings.clone(),
        ytdlp_args: router.ctx.ytdlp_args.clone(),
    };

    if custom_id.starts_with("m:") {
        crate::panel::manejar_interaccion_panel(interaction, custom_id, &ctx_panel, lang).await;
    } else {
        crate::ui::interaccion::manejar_interaccion_auxiliar(
            interaction,
            custom_id,
            &ctx_panel,
            &router.cache_tokens,
            lang,
        )
        .await;
    }
}

pub async fn despachar_modal(
    router: &RouterInteraccionesReal,
    interaction: &Interaction,
    custom_id: &str,
    guild_id: Id<GuildMarker>,
    http: &Arc<HttpClient>,
) {
    let lang = nucleo::idioma_servidor::guild_language(
        &router.ctx.idiomas,
        &router.ctx.data_dir,
        &router.ctx.legacy_languages,
        Some(&guild_id.to_string()),
        None,
    );

    let ctx_panel = ContextoPanel {
        gestor_colas: router.ctx.gestor_colas.clone(),
        gestor_paneles: router.gestor_paneles.clone(),
        idiomas: router.ctx.idiomas.clone(),
        emisor: Arc::new(EmisorRespuestaReal::nuevo(http.clone())),
        cache: router.cache.clone(),
        supabase_url: None,
        gestor_favoritos: router.ctx.gestor_favoritos.clone(),
        driver: router.driver.clone(),
        ytdlp_settings: router.ctx.ytdlp_settings.clone(),
        ytdlp_args: router.ctx.ytdlp_args.clone(),
    };

    if custom_id == crate::ui::componentes::id::VOLUME_MODAL {
        crate::panel::modal::manejar_modal_volumen(interaction, &ctx_panel, lang).await;
    }
}

pub async fn despachar_autocompletado(
    _router: &RouterInteraccionesReal,
    interaction: &Interaction,
    data: &CommandData,
    http: &Arc<HttpClient>,
) {
    let consulta = extraer_consulta_autocompletado(&data.options);
    let opciones = crate::comandos::autocompletar_comando(&data.name, &consulta).await;
    let choices = opciones
        .into_iter()
        .take(25)
        .map(
            |c| twilight_model::application::command::CommandOptionChoice {
                name: c.name,
                name_localizations: None,
                value: twilight_model::application::command::CommandOptionChoiceValue::String(
                    c.value,
                ),
            },
        )
        .collect();

    let interaction_client = http.interaction(interaction.application_id);
    let resp = twilight_model::http::interaction::InteractionResponse {
        kind: twilight_model::http::interaction::InteractionResponseType::ApplicationCommandAutocompleteResult,
        data: Some(twilight_model::http::interaction::InteractionResponseData {
            choices: Some(choices),
            ..Default::default()
        }),
    };
    let _ = interaction_client
        .create_response(interaction.id, &interaction.token, &resp)
        .await;
}
