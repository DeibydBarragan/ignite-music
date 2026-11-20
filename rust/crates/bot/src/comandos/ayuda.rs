use crate::comandos::definiciones::todos_los_cuerpos;
use crate::comandos::{categoria_comando, Contexto};
use crate::respuesta::{responder, EmisorRespuesta, EstadoInteraccion};
use crate::ui::ayuda::{vista_ayuda, EnlacesAyuda, InfoComandoAyuda, StatsAyuda};
use nucleo::formato::formatear_uptime;
use std::collections::HashMap;

const CATEGORY_ORDER: [&str; 3] = ["playback", "queue", "settings"];

fn category_index(cat: Option<&str>) -> usize {
    match cat {
        Some(c) => CATEGORY_ORDER.iter().position(|&x| x == c).unwrap_or(99),
        None => 99,
    }
}

pub async fn ejecutar_help(
    ctx: &Contexto,
    lang: &str,
    servers_count: u64,
    ping_ms: u64,
    uptime_ms: u64,
    emisor: &dyn EmisorRespuesta,
    ei: &EstadoInteraccion,
) {
    let mut defs = todos_los_cuerpos(&ctx.idiomas, 100);
    defs.sort_by(|a, b| {
        let cat_a = category_index(categoria_comando(&a.name));
        let cat_b = category_index(categoria_comando(&b.name));
        cat_a.cmp(&cat_b).then_with(|| a.name.cmp(&b.name))
    });

    let ids_map = ctx.command_ids.read().await;
    let mut info_list = Vec::new();
    let empty_vars = HashMap::new();

    for cmd in defs {
        let key = format!("commands.{}.description", cmd.name);
        let loc = ctx.idiomas.t(lang, &key, &empty_vars);
        let desc = if loc == key { cmd.description } else { loc };
        let cmd_id = ids_map.get(&cmd.name).cloned();
        info_list.push(InfoComandoAyuda {
            name: cmd.name,
            id: cmd_id,
            description: desc,
        });
    }

    let active_queues = ctx.gestor_colas.colas_activas().await.len() as u64;
    let stats = StatsAyuda {
        servers: servers_count,
        active: active_queues,
        uptime: formatear_uptime(uptime_ms),
        ping: ping_ms,
    };

    let invite_url = ctx.config_bot.invite(&ctx.client_id);
    let enlaces = EnlacesAyuda {
        support_server: if ctx.config_bot.support_server.is_empty() {
            None
        } else {
            Some(ctx.config_bot.support_server.clone())
        },
        website: if ctx.config_bot.website.is_empty() {
            None
        } else {
            Some(ctx.config_bot.website.clone())
        },
        invite: if invite_url.is_empty() {
            None
        } else {
            Some(invite_url)
        },
    };

    let payload = vista_ayuda(&ctx.idiomas, lang, &info_list, Some(&stats), Some(&enlaces));
    let _ = responder(emisor, ei, &payload).await;
}
