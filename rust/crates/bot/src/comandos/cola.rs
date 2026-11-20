use crate::comandos::Contexto;
use crate::respuesta::PayloadRespuesta;
use crate::ui::cola::{vista_cola, DatosItemCola};
use crate::ui::componentes::{aviso, DEFAULT_COLOR};
use std::collections::HashMap;

pub async fn ejecutar_cola(
    ctx: &Contexto,
    guild_id: &str,
    lang: &str,
    page_opt: Option<i64>,
) -> PayloadRespuesta {
    let cola_arc = ctx.gestor_colas.obtener_cola(guild_id).await;
    let cola = cola_arc.lock().await;

    if cola.canciones.is_empty() {
        let msg = ctx
            .idiomas
            .t(lang, "commands.nowplaying.no_track", &HashMap::new());
        return aviso(&msg, true, Some(DEFAULT_COLOR));
    }

    let page = page_opt.map(|p| (p - 1).max(0)).unwrap_or(0);
    let items: Vec<DatosItemCola> = cola
        .canciones
        .iter()
        .map(|s| DatosItemCola {
            cancion: s.cancion.clone(),
            requester_id: s.requester_id.clone(),
            requester_es_bot: false,
        })
        .collect();

    let actual = items.first();
    let proximas = if items.len() > 1 { &items[1..] } else { &[] };
    vista_cola(&ctx.idiomas, lang, actual, proximas, page)
}
