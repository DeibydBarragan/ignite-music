//! Funciones auxiliares para la ejecución de comandos (Specs S-22, S-23).

mod controles;
pub use controles::{ejecutar_cmd_seek, ejecutar_cmd_stop};

use crate::comandos::reproducir::reproducir_desde_interaccion;
use crate::interacciones::RouterInteraccionesReal;
use crate::panel::interaccion::ContextoPanel;
use crate::panel::InstantaneaCola;
use crate::permisos::{revisar_voz, InfoCanalVoz, OpcionesRevisarVoz};
use crate::respuesta::{avisar, EmisorRespuestaReal, EstadoInteraccion};
use std::collections::HashMap;
use std::sync::Arc;
use twilight_http::Client as HttpClient;
use twilight_model::id::marker::{ChannelMarker, GuildMarker};
use twilight_model::id::Id;

pub fn requiere_guard_voz(nombre: &str) -> bool {
    matches!(
        nombre,
        "play"
            | "playnext"
            | "search"
            | "pause"
            | "resume"
            | "stop"
            | "skip"
            | "previous"
            | "shuffle"
            | "loop"
            | "volume"
            | "jump"
            | "move"
            | "remove"
            | "clear"
            | "filter"
            | "autoplay"
            | "seek"
    )
}

pub async fn validar_guard_voz(
    router: &RouterInteraccionesReal,
    guild_id: Id<GuildMarker>,
    nombre: &str,
    canal_usuario: Option<&InfoCanalVoz>,
) -> Result<(), &'static str> {
    let cola_arc = router
        .ctx
        .gestor_colas
        .obtener_cola(&guild_id.to_string())
        .await;
    let cola = cola_arc.lock().await;
    let canal_bot: Option<Id<ChannelMarker>> =
        cola.canal_voz.as_deref().and_then(|s| s.parse().ok());
    let unirse = matches!(nombre, "play" | "playnext" | "search");
    let permisos_bot = if let (Some(c), Some(cu)) = (&router.cache, canal_usuario) {
        router
            .ctx
            .client_id
            .parse::<u64>()
            .ok()
            .and_then(|bot_uid| {
                crate::permisos::calcular_permisos_canal_cache(c, Id::new(bot_uid), cu.id)
            })
    } else {
        None
    };
    revisar_voz(
        canal_usuario,
        canal_bot,
        Some(&cola),
        permisos_bot,
        OpcionesRevisarVoz {
            requiere_cola: !unirse,
            unirse,
        },
    )
    .map(|_| ())
}

#[allow(clippy::too_many_arguments)]
pub async fn ejecutar_cmd_play(
    router: &RouterInteraccionesReal,
    guild_id: Id<GuildMarker>,
    nombre: &str,
    solicitante: Option<String>,
    canal_usuario: Option<InfoCanalVoz>,
    opciones: &HashMap<String, serde_json::Value>,
    lang: &str,
    emisor: &EmisorRespuestaReal,
    ei: &EstadoInteraccion,
) {
    let guild = guild_id.to_string();
    let query = opciones
        .get("query")
        .or_else(|| opciones.get("song"))
        .or_else(|| opciones.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let siguiente = nombre == "playnext"
        || opciones
            .get("next")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
    if !query.trim().is_empty() {
        if let Some(proveedor) = &router.proveedor_musica {
            let canal = canal_usuario.as_ref().map(|cu| cu.id.to_string());
            match proveedor
                .reproducir(
                    &guild,
                    query,
                    canal.as_deref(),
                    solicitante.as_deref(),
                    siguiente,
                    false,
                    false,
                )
                .await
            {
                Ok(resultado) => {
                    let mut vars = HashMap::new();
                    let clave = if let Some(lista) = resultado.playlist {
                        vars.insert("count".into(), lista.count.to_string());
                        "commands.play.playlist_success"
                    } else {
                        vars.insert(
                            "title".into(),
                            resultado.song.map(|c| c.title).unwrap_or_default(),
                        );
                        vars.insert("position".into(), resultado.position.to_string());
                        if resultado.started {
                            "commands.play.track_started"
                        } else if siguiente {
                            "commands.playnext.track_queued"
                        } else {
                            "commands.play.track_queued"
                        }
                    };
                    let mensaje = router.ctx.t(lang, clave, &vars);
                    let _ = avisar(emisor, ei, &mensaje, true).await;
                    refrescar_panel_servidor(router, guild_id, lang).await;
                }
                Err((_, codigo, error)) => {
                    tracing::warn!(%guild_id, %codigo, %error, "Falló la reproducción solicitada");
                    let clave = if codigo == "voice_join_failed" {
                        "musicplayer.failed_connect_voice"
                    } else {
                        "musicplayer.track_could_not_play"
                    };
                    let mensaje = router.ctx.t(lang, clave, &HashMap::new());
                    let _ = avisar(emisor, ei, &mensaje, true).await;
                }
            }
            return;
        }
        if let (Some(drv), Some(cu)) = (&router.driver, &canal_usuario) {
            if let Err(err) = drv.join(guild_id.get(), cu.id.get()).await {
                tracing::warn!(%guild_id, error = %err, "No se pudo conectar a voz");
                let mensaje =
                    router
                        .ctx
                        .t(lang, "musicplayer.failed_connect_voice", &HashMap::new());
                let _ = avisar(emisor, ei, &mensaje, true).await;
                return;
            }
            let cola = router.ctx.gestor_colas.obtener_cola(&guild).await;
            cola.lock().await.canal_voz = Some(cu.id.to_string());
        }
    }
    match reproducir_desde_interaccion(&router.ctx, &guild, lang, query, siguiente).await {
        Ok(msg) => {
            let _ = avisar(emisor, ei, &msg, true).await;
            refrescar_panel_servidor(router, guild_id, lang).await;
        }
        Err(e) => {
            let _ = avisar(emisor, ei, &e, true).await;
        }
    }
}

pub fn crear_ctx_panel(router: &RouterInteraccionesReal, http: &Arc<HttpClient>) -> ContextoPanel {
    ContextoPanel {
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
    }
}

pub async fn refrescar_panel_servidor(
    router: &RouterInteraccionesReal,
    guild_id: Id<GuildMarker>,
    lang: &str,
) {
    let cola_arc = router
        .ctx
        .gestor_colas
        .obtener_cola(&guild_id.to_string())
        .await;
    let cola = cola_arc.lock().await;
    if let Some(snap) = InstantaneaCola::desde_cola(&cola) {
        drop(cola);
        router
            .gestor_paneles
            .refrescar_panel(guild_id, snap, false, lang)
            .await;
    }
}
