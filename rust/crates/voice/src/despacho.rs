use crate::despacho_tipos::{disparar_tts, resolver_mensaje_feedback_con_plantillas};
use crate::matcher::coincidir;
use player::Musica;

pub use crate::despacho_tipos::{
    resolver_mensaje_feedback, AjustesVoz, ResultadoDespacho, SolicitudDespacho,
};

pub async fn despachar<M: Musica>(
    musica: &M,
    solicitud: &SolicitudDespacho,
    ajustes: Option<&AjustesVoz>,
) -> ResultadoDespacho {
    if let Some(cfg) = ajustes {
        if !cfg.enabled {
            return ResultadoDespacho::fail("voice_disabled_for_guild", None, None);
        }
    }

    let wake_word = ajustes
        .and_then(|a| a.wake_word.as_deref())
        .unwrap_or("ignite");
    let bindings = ajustes.and_then(|a| a.bindings.as_deref());

    let match_res = coincidir(&solicitud.text, wake_word, bindings);
    if !match_res.coincide {
        return ResultadoDespacho::fail(
            match_res.motivo.as_deref().unwrap_or("unknown"),
            None,
            None,
        );
    }

    let command_id = match_res.comando.unwrap_or_default();
    let query = match_res.consulta.unwrap_or_default();
    let missing_query = match_res.falta_consulta.unwrap_or(false);

    let voice_channel = solicitud
        .channel_id
        .clone()
        .or_else(|| musica.user_voice_channel(&solicitud.guild_id, &solicitud.user_id));
    let estado = musica.estado(&solicitud.guild_id).await.ok();
    let activa = estado.as_ref().is_some_and(|cola| cola.activa);

    if missing_query {
        if command_id == "play" && activa && estado.is_some_and(|cola| cola.pausada) {
            if musica
                .apply_control(&solicitud.guild_id, "resume", None)
                .await
                .is_err()
            {
                return ResultadoDespacho::fail("execution_error", Some("resume"), None);
            }
            disparar_tts(
                solicitud,
                voice_channel.as_deref(),
                "resume",
                "",
                ajustes,
                false,
            );
            let tts = resolver_mensaje_feedback_con_plantillas(
                "resume",
                "",
                ajustes.and_then(|a| a.tts_templates.as_ref()),
            );
            return ResultadoDespacho::ok("resume", "", tts);
        }
        return ResultadoDespacho::fail("missing_argument", Some(&command_id), None);
    }

    if !musica.guild_exists(&solicitud.guild_id) {
        return ResultadoDespacho::fail("guild_not_found", Some(&command_id), Some(&query));
    }

    let has_voice = voice_channel.is_some()
        || musica.user_has_voice_channel(&solicitud.guild_id, &solicitud.user_id);

    let custom_templates = ajustes.and_then(|a| a.tts_templates.as_ref());

    match command_id.as_str() {
        "play" | "play_next" | "enqueue" => {
            if !has_voice {
                return ResultadoDespacho::fail(
                    "user_not_in_voice",
                    Some(&command_id),
                    Some(&query),
                );
            }
            let (skip, pos) = match command_id.as_str() {
                "play" => (true, None),
                "play_next" => (false, Some(1)),
                _ => (false, Some(0)),
            };
            if musica
                .play(
                    &solicitud.guild_id,
                    solicitud.channel_id.as_deref(),
                    &solicitud.user_id,
                    &query,
                    skip,
                    pos,
                )
                .await
                .is_err()
            {
                return ResultadoDespacho::fail("execution_error", Some(&command_id), Some(&query));
            }
            disparar_tts(
                solicitud,
                voice_channel.as_deref(),
                &command_id,
                &query,
                ajustes,
                false,
            );
            let tts =
                resolver_mensaje_feedback_con_plantillas(&command_id, &query, custom_templates);
            ResultadoDespacho::ok(&command_id, &query, tts)
        }
        "pause" | "resume" | "skip" | "previous" | "stop" | "shuffle" | "volume" => {
            if !activa {
                return ResultadoDespacho::fail("no_active_queue", Some(&command_id), Some(&query));
            }
            let val = if command_id == "volume" {
                query.parse::<i64>().ok()
            } else {
                None
            };
            if musica
                .apply_control(&solicitud.guild_id, &command_id, val)
                .await
                .is_err()
            {
                return ResultadoDespacho::fail("execution_error", Some(&command_id), Some(&query));
            }
            disparar_tts(
                solicitud,
                voice_channel.as_deref(),
                &command_id,
                &query,
                ajustes,
                false,
            );
            let tts =
                resolver_mensaje_feedback_con_plantillas(&command_id, &query, custom_templates);
            ResultadoDespacho::ok(&command_id, &query, tts)
        }
        "autoplay_on" | "autoplay_off" => {
            if !activa {
                return ResultadoDespacho::fail("no_active_queue", Some(&command_id), Some(&query));
            }
            let enabled = command_id == "autoplay_on";
            if musica
                .set_autoplay(&solicitud.guild_id, enabled)
                .await
                .is_err()
            {
                return ResultadoDespacho::fail("execution_error", Some(&command_id), Some(&query));
            }
            musica.refresh_panel(&solicitud.guild_id);
            disparar_tts(
                solicitud,
                voice_channel.as_deref(),
                &command_id,
                &query,
                ajustes,
                false,
            );
            let tts =
                resolver_mensaje_feedback_con_plantillas(&command_id, &query, custom_templates);
            ResultadoDespacho::ok(&command_id, &query, tts)
        }
        "queue" => {
            if !activa {
                return ResultadoDespacho::fail("no_active_queue", Some("queue"), Some(&query));
            }
            disparar_tts(
                solicitud,
                voice_channel.as_deref(),
                "queue",
                &query,
                ajustes,
                false,
            );
            let tts = resolver_mensaje_feedback_con_plantillas("queue", &query, custom_templates);
            ResultadoDespacho::ok("queue", &query, tts)
        }
        "hola_que_hace" => {
            disparar_tts(
                solicitud,
                voice_channel.as_deref(),
                "hola_que_hace",
                "",
                ajustes,
                true,
            );
            let tts =
                resolver_mensaje_feedback_con_plantillas("hola_que_hace", "", custom_templates);
            ResultadoDespacho::ok("hola_que_hace", "", tts)
        }
        _ => ResultadoDespacho::fail("unsupported_command", Some(&command_id), Some(&query)),
    }
}
