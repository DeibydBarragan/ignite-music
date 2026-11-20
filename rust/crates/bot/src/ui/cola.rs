use crate::respuesta::PayloadRespuesta;
use crate::ui::componentes::{
    contenedor, id, paginador, separador, texto, vista_base, DEFAULT_COLOR,
};
use medios::Cancion;
use nucleo::formato::{enlace, formatear_duracion};
use nucleo::idioma::Idiomas;
use std::collections::HashMap;

pub const QUEUE_PAGE_SIZE: usize = 10;

#[derive(Debug, Clone)]
pub struct DatosItemCola {
    pub cancion: Cancion,
    pub requester_id: Option<String>,
    pub requester_es_bot: bool,
}

pub fn vista_cola(
    idiomas: &Idiomas,
    lang: &str,
    actual: Option<&DatosItemCola>,
    proximas: &[DatosItemCola],
    pagina: i64,
) -> PayloadRespuesta {
    let empty_vars = HashMap::new();
    let total_proximas = proximas.len();
    let pages = total_proximas.div_ceil(QUEUE_PAGE_SIZE).max(1);
    let safe_page = (pagina.max(0) as usize).min(pages - 1);

    let mut total_segundos: u64 = proximas.iter().map(|p| p.cancion.duracion).sum();
    if let Some(act) = actual {
        total_segundos += act.cancion.duracion;
    }

    let mut componentes = Vec::new();

    // 1. Cabecera
    let mut header_lines = vec![format!(
        "### {}",
        idiomas.t(lang, "buttons.queue", &empty_vars)
    )];
    if let Some(act) = actual {
        let req_str = match &act.requester_id {
            Some(uid) if act.requester_es_bot => {
                format!(" · {}", idiomas.t(lang, "ui.autoplay_added", &empty_vars))
            }
            Some(uid) => format!(" · <@{uid}>"),
            None => String::new(),
        };
        let dur_o_vivo = if act.cancion.es_directo {
            idiomas.t(lang, "ui.live", &empty_vars)
        } else {
            formatear_duracion(act.cancion.duracion)
        };
        let song_link = enlace(&act.cancion.nombre, &act.cancion.url, 70);
        let np_lbl = idiomas.t(lang, "buttonhandler.now_playing", &empty_vars);
        header_lines.push(format!("{np_lbl}: {song_link} · {dur_o_vivo}{req_str}"));
    }
    componentes.push(texto(header_lines.join("\n")));

    // 2. Próximas canciones
    if !proximas.is_empty() {
        let start = safe_page * QUEUE_PAGE_SIZE;
        let end = (start + QUEUE_PAGE_SIZE).min(proximas.len());
        let slice = &proximas[start..end];

        let mut lines = Vec::new();
        for (i, item) in slice.iter().enumerate() {
            let num = start + i + 1;
            let num_str = format!("{num:>2}.");
            let req_str = match &item.requester_id {
                Some(uid) if item.requester_es_bot => {
                    format!(" · *{}*", idiomas.t(lang, "ui.autoplay_added", &empty_vars))
                }
                Some(uid) => format!(" · <@{uid}>"),
                None => String::new(),
            };
            let dur_o_vivo = if item.cancion.es_directo {
                idiomas.t(lang, "ui.live", &empty_vars)
            } else {
                formatear_duracion(item.cancion.duracion)
            };
            let song_link = enlace(&item.cancion.nombre, &item.cancion.url, 50);
            lines.push(format!("`{num_str}` {song_link} · {dur_o_vivo}{req_str}"));
        }

        componentes.push(separador(false));
        let mut count_vars = HashMap::new();
        count_vars.insert("count".to_string(), total_proximas.to_string());
        let up_title = idiomas.t(lang, "buttonhandler.upcoming_songs", &count_vars);
        componentes.push(texto(format!("**{up_title}**\n{}", lines.join("\n"))));
    } else {
        let no_songs = idiomas.t(lang, "commands.nowplaying.footer_no_songs", &empty_vars);
        componentes.push(texto(format!("-# {no_songs}")));
    }

    // 3. Pie
    let total_count = total_proximas + if actual.is_some() { 1 } else { 0 };
    let mut total_vars = HashMap::new();
    total_vars.insert("count".to_string(), total_count.to_string());
    let total_lbl = idiomas.t(lang, "buttonhandler.total_songs", &total_vars);
    let total_dur = formatear_duracion(total_segundos);

    let mut page_vars = HashMap::new();
    page_vars.insert("page".to_string(), (safe_page + 1).to_string());
    page_vars.insert("pages".to_string(), pages.to_string());
    let page_lbl = idiomas.t(lang, "ui.page", &page_vars);

    componentes.push(texto(format!("-# {total_lbl} · {total_dur} · {page_lbl}")));

    // 4. Paginador
    if pages > 1 {
        let atras = idiomas.t(lang, "ui.back", &empty_vars);
        let sig = idiomas.t(lang, "ui.next", &empty_vars);
        componentes.push(paginador(
            id::QUEUE_PAGE,
            safe_page as i64,
            pages,
            &atras,
            &sig,
        ));
    }

    let c = contenedor(Some(DEFAULT_COLOR), componentes);
    vista_base(vec![c], true)
}
