use crate::respuesta::PayloadRespuesta;
use crate::ui::componentes::{
    contenedor, id, paginador, separador, texto, vista_base, DEFAULT_COLOR,
};
use medios::letras::dividir_letras;
use nucleo::formato::{escapar_markdown, truncar};
use nucleo::idioma::Idiomas;
use std::collections::HashMap;

#[allow(clippy::too_many_arguments)]
pub fn vista_letras(
    idiomas: &Idiomas,
    lang: &str,
    title: &str,
    artist: Option<&str>,
    lyrics: &str,
    source: Option<&str>,
    token: Option<&str>,
    page: i64,
) -> PayloadRespuesta {
    let empty_vars = HashMap::new();
    let pages = dividir_letras(lyrics);
    let total_pages = pages.len();
    let safe_page = (page.max(0) as usize).min(total_pages - 1);

    let mut componentes = Vec::new();

    // 1. Cabecera
    let tit_escaped = escapar_markdown(&truncar(title, 80));
    let subtitle = match artist {
        Some(a) if !a.trim().is_empty() => {
            let art_escaped = escapar_markdown(&truncar(a, 60));
            format!("{tit_escaped} · {art_escaped}")
        }
        _ => tit_escaped,
    };
    let title_lbl = idiomas.t(lang, "buttonhandler.lyrics_title", &empty_vars);
    componentes.push(texto(format!("### {title_lbl}\n-# {subtitle}")));

    // 2. Separador
    componentes.push(separador(false));

    // 3. Contenido de la página
    let raw_page = &pages[safe_page];
    let escaped = escapar_markdown(raw_page);
    let mut page_lines = Vec::new();
    for line in escaped.lines() {
        if line.starts_with('#') || line.starts_with('-') {
            page_lines.push(format!("\\{line}"));
        } else {
            page_lines.push(line.to_string());
        }
    }
    componentes.push(texto(page_lines.join("\n")));

    // 4. Pie
    let mut footer_parts = Vec::new();
    if let Some(s) = source {
        if !s.is_empty() {
            footer_parts.push(s.to_string());
        }
    }
    if total_pages > 1 {
        let mut page_vars = HashMap::new();
        page_vars.insert("page".to_string(), (safe_page + 1).to_string());
        page_vars.insert("pages".to_string(), total_pages.to_string());
        footer_parts.push(idiomas.t(lang, "ui.page", &page_vars));
    }
    if !footer_parts.is_empty() {
        componentes.push(texto(format!("-# {}", footer_parts.join(" · "))));
    }

    // 5. Paginador
    if total_pages > 1 {
        if let Some(tok) = token {
            let atras = idiomas.t(lang, "ui.back", &empty_vars);
            let sig = idiomas.t(lang, "ui.next", &empty_vars);
            let prefix = format!("{}{tok}:", id::LYRICS_PAGE);
            componentes.push(paginador(
                &prefix,
                safe_page as i64,
                total_pages,
                &atras,
                &sig,
            ));
        }
    }

    let c = contenedor(Some(DEFAULT_COLOR), componentes);
    vista_base(vec![c], true)
}
