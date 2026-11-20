use super::instantanea::InstantaneaCola;
use super::interaccion::ContextoPanel;
use crate::respuesta::{avisar, EstadoInteraccion};
use crate::ui::panel::vista_reproductor;
use std::collections::HashMap;
use twilight_model::application::interaction::Interaction;
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use twilight_model::id::Id;

pub async fn manejar_favorito(
    cola: &player::Cola,
    ctx: &ContextoPanel,
    ei: &EstadoInteraccion,
    user_id: &str,
) {
    let Some(cancion) = cola.canciones.first() else {
        let _ = avisar(
            &*ctx.emisor,
            ei,
            "No hay ninguna canción reproduciéndose.",
            true,
        )
        .await;
        return;
    };

    let Some(ref favoritos) = ctx.gestor_favoritos else {
        let _ = avisar(
            &*ctx.emisor,
            ei,
            "Base de datos no disponible temporalmente.",
            true,
        )
        .await;
        return;
    };

    let c = &cancion.cancion;
    let cancion_fav = crate::datos::CancionFavorito {
        song_id: if !c.id.is_empty() {
            c.id.clone()
        } else {
            c.url.clone()
        },
        title: c.nombre.clone(),
        artist: if c.artista.is_empty() {
            "Desconocido".into()
        } else {
            c.artista.clone()
        },
        album_art: c.miniatura.clone(),
        url: c.url.clone(),
        duration: c.duracion as u32,
        source: if c.fuente.is_empty() {
            "youtube".into()
        } else {
            c.fuente.clone()
        },
    };

    match favoritos.handle_favorite(user_id, &cancion_fav).await {
        Ok(crate::datos::AccionFavorito::Eliminado(titulo)) => {
            let msg = format!("💔 **{titulo}** se ha eliminado de tus favoritos.");
            let _ = avisar(&*ctx.emisor, ei, &msg, true).await;
        }
        Ok(crate::datos::AccionFavorito::Agregado(titulo)) => {
            let msg = format!(
                "⭐ ¡**{titulo}** guardada en tus favoritos!\nYa está en tu catálogo web para escucharla cuando quieras."
            );
            let _ = avisar(&*ctx.emisor, ei, &msg, true).await;
        }
        Err(_) => {
            let _ = avisar(
                &*ctx.emisor,
                ei,
                "Ocurrió un error al actualizar tus favoritos.",
                true,
            )
            .await;
        }
    }
}

pub async fn ejecutar_nowplaying(interaction: &Interaction, ctx: &ContextoPanel, lang: &str) {
    let Some(guild_id) = interaction.guild_id else {
        return;
    };
    let guild_str = guild_id.to_string();
    let cola_arc = ctx.gestor_colas.obtener_cola(&guild_str).await;
    let cola = cola_arc.lock().await;
    let ei = EstadoInteraccion::desde_interaccion(interaction);

    if !cola.esta_activa() {
        let msg = ctx
            .idiomas
            .t(lang, "commands.nowplaying.no_track", &HashMap::new());
        let _ = avisar(&*ctx.emisor, &ei, &msg, false).await;
        return;
    }

    if let Some(snap) = InstantaneaCola::desde_cola(&cola) {
        let payload = vista_reproductor(&ctx.idiomas, lang, &snap);
        let resp = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(InteractionResponseData {
                content: payload.content,
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
        if let Some(canal_texto) = interaction.channel.as_ref().map(|c| c.id) {
            ctx.gestor_paneles
                .reemplazar_panel(guild_id, canal_texto, Id::new(ei.id.get()), lang)
                .await;
        }
    }
}
