use crate::respuesta::PayloadRespuesta;
use crate::ui::componentes::{contenedor, fila, id, separador, texto, vista_base, DEFAULT_COLOR};
use medios::Cancion;
use nucleo::formato::{enlace, escapar_markdown, formatear_duracion, truncar};
use nucleo::idioma::Idiomas;
use std::collections::HashMap;
use twilight_model::channel::message::component::{
    Component, SelectMenu, SelectMenuOption, SelectMenuType,
};

pub fn vista_busqueda(
    idiomas: &Idiomas,
    lang: &str,
    query: &str,
    results: &[Cancion],
    token: &str,
) -> PayloadRespuesta {
    let empty_vars = HashMap::new();
    let mut query_vars = HashMap::new();
    query_vars.insert("query".to_string(), escapar_markdown(&truncar(query, 80)));
    let title_line = idiomas.t(lang, "commands.search.title", &query_vars);

    let mut lines = Vec::new();
    let mut opciones = Vec::new();

    for (index, song) in results.iter().enumerate() {
        let dur_o_vivo = if song.es_directo {
            idiomas.t(lang, "ui.live", &empty_vars)
        } else {
            formatear_duracion(song.duracion)
        };
        let mut meta_parts = Vec::new();
        if !song.artista.is_empty() {
            meta_parts.push(escapar_markdown(&truncar(&song.artista, 40)));
        }
        meta_parts.push(dur_o_vivo.clone());
        let meta_str = meta_parts.join(" · ");
        let song_link = enlace(&song.nombre, &song.url, 70);
        lines.push(format!("`{}.` {song_link}\n-# {meta_str}", index + 1));

        let mut desc_parts = Vec::new();
        if !song.artista.is_empty() {
            desc_parts.push(song.artista.clone());
        }
        desc_parts.push(dur_o_vivo);
        let opt_desc = truncar(&desc_parts.join(" · "), 100);

        let opt_label = truncar(&format!("{}. {}", index + 1, song.nombre), 100);
        opciones.push(SelectMenuOption {
            default: false,
            description: Some(opt_desc),
            emoji: None,
            label: opt_label,
            value: index.to_string(),
        });
    }

    let mut componentes = Vec::new();
    componentes.push(texto(format!("### {title_line}\n{}", lines.join("\n"))));
    componentes.push(separador(false));

    let placeholder = truncar(
        &idiomas.t(lang, "commands.search.select_description", &empty_vars),
        150,
    );

    componentes.push(fila(vec![Component::SelectMenu(SelectMenu {
        channel_types: None,
        custom_id: format!("{}{token}", id::SEARCH),
        default_values: None,
        disabled: false,
        id: None,
        kind: SelectMenuType::Text,
        max_values: None,
        min_values: None,
        options: Some(opciones),
        placeholder: Some(placeholder),
        required: None,
    })]));

    let c = contenedor(Some(DEFAULT_COLOR), componentes);
    vista_base(vec![c], true)
}
