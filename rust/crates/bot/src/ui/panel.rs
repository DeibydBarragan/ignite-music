pub use super::panel_filas::{etiqueta_bucle, etiqueta_filtro, filas_reproductor};

use crate::panel::InstantaneaCola;
use crate::respuesta::PayloadRespuesta;
use crate::ui::componentes::{contenedor, separador, texto, titular, vista_base, DEFAULT_COLOR};
use medios::Cancion;
use nucleo::formato::{barra_progreso, enlace, escapar_markdown, formatear_duracion, truncar};
use nucleo::idioma::Idiomas;
use std::collections::HashMap;

pub fn vista_reproductor(
    idiomas: &Idiomas,
    lang: &str,
    estado: &InstantaneaCola,
) -> PayloadRespuesta {
    let empty_vars = HashMap::new();
    let mut c = Vec::new();

    // 1. Cabecera
    let status = if estado.pausada {
        idiomas.t(lang, "commands.nowplaying.status_paused", &empty_vars)
    } else {
        idiomas.t(lang, "commands.nowplaying.title", &empty_vars)
    };

    let duracion_o_vivo = if estado.cancion.es_directo {
        idiomas.t(lang, "ui.live", &empty_vars)
    } else {
        formatear_duracion(estado.cancion.duracion)
    };

    let meta = if estado.cancion.artista.is_empty() {
        duracion_o_vivo
    } else {
        let artista = escapar_markdown(&truncar(&estado.cancion.artista, 60));
        format!("{artista} · {duracion_o_vivo}")
    };

    let link_cancion = enlace(&estado.cancion.nombre, &estado.cancion.url, 90);
    let titular_texto = format!("-# {status}\n### {link_cancion}\n{meta}");
    titular(&mut c, titular_texto, estado.cancion.miniatura.as_deref());

    // 2. Barra de progreso
    if !estado.cancion.es_directo && estado.cancion.duracion > 0 {
        let position = estado.tiempo_actual.min(estado.cancion.duracion);
        let pos_str = formatear_duracion(position);
        let tot_str = formatear_duracion(estado.cancion.duracion);
        let barra = barra_progreso(position, estado.cancion.duracion, 16);
        c.push(texto(format!("`{pos_str}` {barra} `{tot_str}`")));
    }

    // 3. Detalles
    let req_str = match &estado.requester_id {
        Some(uid) if !estado.requester_es_bot => {
            let label = idiomas.t(lang, "commands.nowplaying.requested_by", &empty_vars);
            format!("{label}: <@{uid}>")
        }
        _ => idiomas.t(lang, "ui.autoplay_added", &empty_vars),
    };

    let mut vol_vars = HashMap::new();
    vol_vars.insert("volume".to_string(), estado.volumen.to_string());
    let vol_str = idiomas.t(lang, "commands.nowplaying.volume", &vol_vars);
    let loop_str = etiqueta_bucle(idiomas, lang, estado.modo_repeticion);
    let auto_key = if estado.autoplay {
        "buttons.autoplay_on"
    } else {
        "buttons.autoplay_off"
    };
    let auto_str = idiomas.t(lang, auto_key, &empty_vars);

    let mut det_parts = vec![req_str, vol_str, loop_str, auto_str];
    if let Some(ref f) = estado.filtro {
        let mut filt_vars = HashMap::new();
        filt_vars.insert(
            "filter".to_string(),
            etiqueta_filtro(idiomas, lang, Some(f)),
        );
        det_parts.push(idiomas.t(lang, "ui.filter", &filt_vars));
    }
    c.push(texto(format!("-# {}", det_parts.join(" · "))));

    // 4. Siguiente canción
    let cola_linea = if let Some(next) = estado.proximas.first() {
        let next_req = match &estado.proxima_requester_id {
            Some(uid) if !estado.proxima_requester_es_bot => format!(" · <@{uid}>"),
            Some(_) => format!(" · {}", idiomas.t(lang, "ui.autoplay_added", &empty_vars)),
            None => String::new(),
        };
        let next_title = enlace(&next.nombre, &next.url, 60);
        let mut count_vars = HashMap::new();
        count_vars.insert("count".to_string(), estado.proximas.len().to_string());
        let footer = idiomas.t(lang, "commands.nowplaying.footer_more_songs", &count_vars);
        let dur = formatear_duracion(estado.duracion_proximas);
        let next_label = idiomas.t(lang, "commands.nowplaying.next_song", &empty_vars);
        format!("{next_label}: {next_title}{next_req}\n-# {footer} · {dur}")
    } else {
        format!(
            "-# {}",
            idiomas.t(lang, "commands.nowplaying.footer_no_songs", &empty_vars)
        )
    };
    c.push(texto(cola_linea));

    // 5. Separador
    c.push(separador(false));

    // 6. Filas de controles y filtros
    c.extend(filas_reproductor(idiomas, lang, estado));

    vista_base(vec![contenedor(Some(DEFAULT_COLOR), c)], false)
}

pub fn vista_terminada(
    idiomas: &Idiomas,
    lang: &str,
    titulo: &str,
    descripcion: Option<&str>,
    cancion: Option<&Cancion>,
) -> PayloadRespuesta {
    let empty_vars = HashMap::new();
    let tit_limpio = if titulo.starts_with("commands.") || titulo.starts_with("music") {
        idiomas.t(lang, titulo, &empty_vars)
    } else {
        titulo.to_string()
    };
    let mut lines = vec![format!("### {tit_limpio}")];
    if let Some(desc) = descripcion {
        let desc_limpio = if desc.starts_with("commands.") || desc.starts_with("music") {
            idiomas.t(lang, desc, &empty_vars)
        } else {
            desc.to_string()
        };
        lines.push(desc_limpio);
    }
    if let Some(song) = cancion {
        lines.push(format!("-# {}", enlace(&song.nombre, &song.url, 90)));
    }
    let c = contenedor(Some(DEFAULT_COLOR), vec![texto(lines.join("\n"))]);
    vista_base(vec![c], false)
}
