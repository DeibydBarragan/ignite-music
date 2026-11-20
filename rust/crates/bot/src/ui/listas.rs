use crate::respuesta::PayloadRespuesta;
use crate::ui::componentes::{contenedor, separador, texto, vista_base, DEFAULT_COLOR};
use nucleo::formato::{escapar_markdown, formatear_duracion, truncar};
use nucleo::idioma::Idiomas;
use std::collections::HashMap;

pub const PLAYLISTS_PAGE_SIZE: usize = 10;

#[derive(Debug, Clone)]
pub struct ItemPlaylistUsuario {
    pub name: String,
    pub url: String,
    pub source: String,
    pub track_count: usize,
    pub duration: u64,
}

pub fn vista_playlists(
    idiomas: &Idiomas,
    lang: &str,
    playlists: &[ItemPlaylistUsuario],
    total: usize,
    page: i64,
    filter: Option<&str>,
) -> PayloadRespuesta {
    let empty_vars = HashMap::new();
    let pages = playlists.len().div_ceil(PLAYLISTS_PAGE_SIZE).max(1);
    let safe_page = (page.max(0) as usize).min(pages - 1);

    let mut componentes = Vec::new();

    // 1. Título
    let title_line = if let Some(f) = filter {
        let mut vars = HashMap::new();
        vars.insert("filter".to_string(), truncar(f, 40));
        vars.insert("count".to_string(), total.to_string());
        idiomas.t(lang, "ui.playlists_title_query", &vars)
    } else {
        let mut vars = HashMap::new();
        vars.insert("count".to_string(), total.to_string());
        idiomas.t(lang, "ui.playlists_title", &vars)
    };
    componentes.push(texto(format!("### {title_line}")));

    // 2. Lista o aviso vacío
    if playlists.is_empty() {
        let empty_lbl = idiomas.t(lang, "ui.playlists_empty", &empty_vars);
        componentes.push(texto(format!("-# {empty_lbl}")));
    } else {
        let start = safe_page * PLAYLISTS_PAGE_SIZE;
        let end = (start + PLAYLISTS_PAGE_SIZE).min(playlists.len());
        let slice = &playlists[start..end];

        let mut lines = Vec::new();
        for (index, pl) in slice.iter().enumerate() {
            let num = start + index + 1;
            let num_str = format!("{num:>2}.");
            let name_str = escapar_markdown(&truncar(&pl.name, 60));
            let dur_str = formatear_duracion(pl.duration);

            let mut line_vars = HashMap::new();
            line_vars.insert("source".to_string(), pl.source.clone());
            line_vars.insert("count".to_string(), pl.track_count.to_string());
            line_vars.insert("duration".to_string(), dur_str);
            let detail = idiomas.t(lang, "ui.playlists_line", &line_vars);

            lines.push(format!("`{num_str}` **{name_str}**\n-# {detail}"));
        }
        componentes.push(separador(false));
        componentes.push(texto(lines.join("\n")));
    }

    // 3. Pie
    let mut page_vars = HashMap::new();
    page_vars.insert("page".to_string(), (safe_page + 1).to_string());
    page_vars.insert("pages".to_string(), pages.to_string());
    let page_lbl = idiomas.t(lang, "ui.page", &page_vars);
    let hint_lbl = idiomas.t(lang, "ui.playlists_hint", &empty_vars);
    componentes.push(texto(format!("-# {page_lbl}\n-# {hint_lbl}")));

    let c = contenedor(Some(DEFAULT_COLOR), componentes);
    vista_base(vec![c], true)
}
