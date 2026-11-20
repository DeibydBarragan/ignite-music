//! Despacho y ejecución real de comandos de barra (Specs S-22, S-23, S-24, S-25).

use crate::interacciones::{extraer_opciones, RouterInteraccionesReal};
use crate::interacciones_comandos_aux::*;
use crate::permisos::{canal_voz_usuario_desde_cache, InfoCanalVoz};
use crate::respuesta::{avisar, diferir, responder, EmisorRespuestaReal, EstadoInteraccion};
use std::collections::HashMap;
use std::sync::Arc;
use twilight_http::Client as HttpClient;
use twilight_model::application::interaction::application_command::CommandData;
use twilight_model::application::interaction::Interaction;
use twilight_model::id::marker::GuildMarker;
use twilight_model::id::Id;

pub async fn ejecutar_comando_completo(
    router: &RouterInteraccionesReal,
    interaction: &Interaction,
    data: &CommandData,
    guild_id: Id<GuildMarker>,
    http: &Arc<HttpClient>,
) {
    let emisor = EmisorRespuestaReal::nuevo(http.clone());
    let mut ei = EstadoInteraccion::desde_interaccion(interaction);
    let nombre = data.name.as_str();
    let _turno_voz = if matches!(nombre, "play" | "playnext" | "stop") {
        if let Err(err) = diferir(&emisor, &mut ei).await {
            tracing::warn!(comando = nombre, error = %err, "No se pudo reconocer la interacción");
            return;
        }
        Some(router.tomar_turno_voz(guild_id.get()).await)
    } else {
        None
    };
    let lang = nucleo::idioma_servidor::guild_language(
        &router.ctx.idiomas,
        &router.ctx.data_dir,
        &router.ctx.legacy_languages,
        Some(&guild_id.to_string()),
        None,
    );

    let opciones = extraer_opciones(&data.options);

    let canal_usuario = router
        .cache
        .as_ref()
        .and_then(|c| {
            interaction
                .author_id()
                .and_then(|uid| canal_voz_usuario_desde_cache(c, guild_id, uid))
        })
        .map(|cid| InfoCanalVoz {
            id: cid,
            lleno: false,
        });

    if requiere_guard_voz(nombre) {
        if let Err(err_key) =
            validar_guard_voz(router, guild_id, nombre, canal_usuario.as_ref()).await
        {
            let msg = router.ctx.idiomas.t(lang, err_key, &HashMap::new());
            let _ = avisar(&emisor, &ei, &msg, true).await;
            return;
        }
    }

    match nombre {
        "play" | "playnext" => {
            ejecutar_cmd_play(
                router,
                guild_id,
                nombre,
                interaction.author_id().map(|id| id.to_string()),
                canal_usuario,
                &opciones,
                lang,
                &emisor,
                &ei,
            )
            .await;
        }
        "nowplaying" => {
            let ctx_panel = crear_ctx_panel(router, http);
            crate::panel::ejecutar_nowplaying(interaction, &ctx_panel, lang).await;
        }
        "queue" => {
            let page = opciones.get("page").and_then(|v| v.as_i64());
            let payload = crate::comandos::cola::ejecutar_cola(
                &router.ctx,
                &guild_id.to_string(),
                lang,
                page,
            )
            .await;
            let _ = responder(&emisor, &ei, &payload).await;
        }
        "help" => {
            let servers = router
                .cache
                .as_ref()
                .map(|c| c.iter().guilds().count() as u64)
                .unwrap_or(1);
            let uptime_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let ping_ms = router.latencia.load(std::sync::atomic::Ordering::Relaxed);
            crate::comandos::ayuda::ejecutar_help(
                &router.ctx,
                lang,
                servers,
                ping_ms,
                uptime_ms,
                &emisor,
                &ei,
            )
            .await;
        }
        "language" => {
            crate::comandos::idioma::ejecutar_language(&router.ctx, lang, &emisor, &ei).await;
        }
        "lyrics" => {
            let song = opciones
                .get("song")
                .or_else(|| opciones.get("query"))
                .and_then(|v| v.as_str());
            crate::comandos::letras::ejecutar_lyrics(
                &router.ctx,
                &guild_id.to_string(),
                lang,
                song,
                &router.cache_tokens,
                None,
                &emisor,
                &ei,
            )
            .await;
        }
        "search" => {
            let query = opciones.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let user_id = interaction.author_id().unwrap_or_else(|| Id::new(1));
            crate::comandos::buscar::ejecutar_search(
                &router.ctx,
                &guild_id.to_string(),
                user_id,
                lang,
                query,
                &router.cache_tokens,
                &emisor,
                &ei,
            )
            .await;
        }
        "playlists" => {
            let filter = opciones
                .get("nombre")
                .or_else(|| opciones.get("filter"))
                .and_then(|v| v.as_str());
            let to_play = opciones
                .get("reproducir")
                .or_else(|| opciones.get("play"))
                .and_then(|v| v.as_str());
            let page = opciones.get("page").and_then(|v| v.as_i64());
            let user_id = interaction
                .author_id()
                .map(|u| u.to_string())
                .unwrap_or_default();
            let mut playlists_cargadas = Vec::new();
            if let Some(ref gl) = router.ctx.gestor_listas {
                if let Ok(pl) = gl.fetch_user_playlists(&user_id, 50).await {
                    playlists_cargadas = pl
                        .into_iter()
                        .map(|p| {
                            let duration: u64 = p
                                .tracks
                                .iter()
                                .filter_map(|t| t.get("duration").and_then(|d| d.as_u64()))
                                .sum();
                            crate::ui::listas::ItemPlaylistUsuario {
                                name: p.name,
                                source: p.source,
                                url: p.url,
                                track_count: p.tracks.len(),
                                duration,
                            }
                        })
                        .collect();
                }
            }
            crate::comandos::listas::ejecutar_playlists(
                &router.ctx,
                &guild_id.to_string(),
                lang,
                filter,
                to_play,
                page,
                &playlists_cargadas,
                &emisor,
                &ei,
            )
            .await;
        }
        "new-playlist" => {
            let name = opciones
                .get("nombre")
                .or_else(|| opciones.get("name"))
                .and_then(|v| v.as_str());
            let url = opciones.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let user_id = interaction
                .author_id()
                .map(|u| u.to_string())
                .unwrap_or_default();
            crate::comandos::listas::ejecutar_new_playlist(
                &router.ctx,
                &user_id,
                url,
                name,
                &emisor,
                &ei,
            )
            .await;
        }
        "seek" => {
            ejecutar_cmd_seek(router, guild_id, &opciones, lang, &emisor, &ei).await;
        }
        "stop" => {
            ejecutar_cmd_stop(router, guild_id, lang, &emisor, &ei).await;
        }
        _ => {
            match crate::comandos::despachar_comando(
                &router.ctx,
                &guild_id.to_string(),
                nombre,
                &opciones,
            )
            .await
            {
                Ok(Some(msg)) => {
                    let _ = avisar(&emisor, &ei, &msg, true).await;
                    refrescar_panel_servidor(router, guild_id, lang).await;
                }
                Ok(None) => {}
                Err(e) => {
                    let _ = avisar(&emisor, &ei, &e, true).await;
                }
            }
        }
    }
}
