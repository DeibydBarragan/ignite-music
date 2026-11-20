use crate::comandos::Contexto;
use crate::respuesta::{responder, EmisorRespuesta, EstadoInteraccion};
use crate::ui::componentes::{aviso, DEFAULT_COLOR};
use crate::ui::listas::{vista_playlists, ItemPlaylistUsuario};
use twilight_model::channel::message::MessageFlags;
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};

pub use super::listas_crear::ejecutar_new_playlist;

pub fn buscar_playlist<'a>(
    playlists: &'a [ItemPlaylistUsuario],
    query: &str,
) -> Option<&'a ItemPlaylistUsuario> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    playlists
        .iter()
        .find(|pl| pl.url.to_lowercase() == q)
        .or_else(|| playlists.iter().find(|pl| pl.name.to_lowercase() == q))
        .or_else(|| {
            playlists
                .iter()
                .find(|pl| pl.name.to_lowercase().contains(&q))
        })
}

#[allow(clippy::too_many_arguments)]
pub async fn ejecutar_playlists(
    ctx: &Contexto,
    _guild_id: &str,
    lang: &str,
    filter_opt: Option<&str>,
    to_play_opt: Option<&str>,
    page_opt: Option<i64>,
    playlists_usuario: &[ItemPlaylistUsuario],
    emisor: &dyn EmisorRespuesta,
    ei: &EstadoInteraccion,
) {
    let mut estado_diferido = ei.clone();
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
        estado_diferido.diferida = true;
    }

    if let Some(to_play) = to_play_opt.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let match_pl = buscar_playlist(playlists_usuario, to_play);
        match match_pl {
            None => {
                let names = playlists_usuario
                    .iter()
                    .take(5)
                    .map(|pl| format!("• {}", pl.name))
                    .collect::<Vec<_>>()
                    .join("\n");
                let msg = if !names.is_empty() {
                    format!(
                        "No encontré ninguna playlist con \"{to_play}\".\nTus playlists:\n{names}"
                    )
                } else {
                    format!("No encontré ninguna playlist con \"{to_play}\". Aún no tienes playlists: usa `/new-playlist` para importar una.")
                };
                let payload = aviso(&msg, true, Some(DEFAULT_COLOR));
                let _ = responder(emisor, &estado_diferido, &payload).await;
            }
            Some(pl) => {
                let msg = format!("🎵 Reproduciendo **{}**...", pl.name);
                let payload = aviso(&msg, true, Some(DEFAULT_COLOR));
                let _ = responder(emisor, &estado_diferido, &payload).await;
            }
        }
        return;
    }

    let page = page_opt.map(|p| (p - 1).max(0)).unwrap_or(0);
    let filter = filter_opt.map(|s| s.trim()).filter(|s| !s.is_empty());

    let filtered: Vec<ItemPlaylistUsuario> = match filter {
        Some(f) => {
            let q = f.to_lowercase();
            playlists_usuario
                .iter()
                .filter(|pl| {
                    pl.name.to_lowercase().contains(&q) || pl.source.to_lowercase().contains(&q)
                })
                .cloned()
                .collect()
        }
        None => playlists_usuario.to_vec(),
    };

    let payload = vista_playlists(
        &ctx.idiomas,
        lang,
        &filtered,
        playlists_usuario.len(),
        page,
        filter,
    );
    let _ = responder(emisor, &estado_diferido, &payload).await;
}
