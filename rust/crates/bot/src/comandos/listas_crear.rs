use crate::comandos::Contexto;
use crate::respuesta::{responder, EmisorRespuesta, EstadoInteraccion};
use crate::ui::componentes::{aviso, DEFAULT_COLOR};
use medios::playlists::{derive_name, resolve_playlist};
use medios::ytdlp_args::resolve_binary;
use twilight_model::channel::message::MessageFlags;
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};

fn es_url_http(url: &str) -> bool {
    let lower = url.trim().to_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn es_url_soundcloud(url: &str) -> bool {
    url.to_lowercase().contains("soundcloud.com")
}

pub async fn ejecutar_new_playlist(
    ctx: &Contexto,
    user_id: &str,
    url: &str,
    custom_name_opt: Option<&str>,
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

    if !es_url_http(url) {
        let payload = aviso(
            "Pega un enlace válido de Spotify o YouTube.",
            true,
            Some(DEFAULT_COLOR),
        );
        let _ = responder(emisor, &estado_diferido, &payload).await;
        return;
    }

    if es_url_soundcloud(url) {
        let payload = aviso(
            "SoundCloud ya no está soportado. Usa un enlace de Spotify o YouTube.",
            true,
            Some(DEFAULT_COLOR),
        );
        let _ = responder(emisor, &estado_diferido, &payload).await;
        return;
    }

    let settings = ctx.ytdlp_settings.clone();
    let binario = resolve_binary(&settings);
    let limiter = settings.limiter.clone();
    let res = resolve_playlist(
        url,
        custom_name_opt,
        None,
        None,
        &binario,
        None,
        &settings,
        &[],
        &limiter,
    )
    .await;

    match res {
        Err(_) => {
            let payload = aviso("Error al resolver la playlist.", true, Some(DEFAULT_COLOR));
            let _ = responder(emisor, &estado_diferido, &payload).await;
        }
        Ok(resolved) => {
            if resolved.tracks.is_empty() {
                let payload = aviso(
                    "No encontré canciones en ese enlace. Revisa que la playlist sea pública.",
                    true,
                    Some(DEFAULT_COLOR),
                );
                let _ = responder(emisor, &estado_diferido, &payload).await;
                return;
            }
            let name = derive_name(url, custom_name_opt.or(Some(&resolved.name)));
            let note = if resolved.truncated {
                " (recortada al máximo permitido)"
            } else {
                ""
            };

            let Some(ref gl) = ctx.gestor_listas else {
                let payload = aviso(
                    "Resolví la playlist pero no pude guardarla en la base de datos. Inténtalo de nuevo.",
                    true,
                    Some(DEFAULT_COLOR),
                );
                let _ = responder(emisor, &estado_diferido, &payload).await;
                return;
            };

            let tracks_val: Vec<serde_json::Value> = resolved
                .tracks
                .iter()
                .filter_map(|t| serde_json::to_value(t).ok())
                .collect();

            let cover = resolved.tracks.first().map(|t| t.album_art.clone());

            let input = crate::datos::UserPlaylistInput {
                user_id: user_id.to_string(),
                name: Some(name.clone()),
                source: Some(resolved.source.clone()),
                url: url.to_string(),
                cover,
                tracks: tracks_val,
                synced_at: Some(chrono::Utc::now().to_rfc3339()),
            };

            match gl.upsert_user_playlist(&input).await {
                Ok(upsert_res) => {
                    let verb = if upsert_res.created {
                        "guardada"
                    } else {
                        "actualizada"
                    };
                    let msg = format!(
                        "✅ Playlist **{}** {verb}: {} temas{note}.\nVela con `/playlists` o suénala con `/playlists reproducir:{}`.",
                        upsert_res.playlist.name,
                        upsert_res.playlist.tracks.len(),
                        upsert_res.playlist.name
                    );
                    let payload = aviso(&msg, true, Some(DEFAULT_COLOR));
                    let _ = responder(emisor, &estado_diferido, &payload).await;
                }
                Err(e) => {
                    tracing::error!("Failed to save playlist for {user_id}: {:?}", e);
                    let payload = aviso(
                        "Resolví la playlist pero no pude guardarla en la base de datos. Inténtalo de nuevo.",
                        true,
                        Some(DEFAULT_COLOR),
                    );
                    let _ = responder(emisor, &estado_diferido, &payload).await;
                }
            }
        }
    }
}
