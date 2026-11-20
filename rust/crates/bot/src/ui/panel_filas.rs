use crate::panel::InstantaneaCola;
use crate::ui::componentes::{boton, fila, id};
use nucleo::idioma::Idiomas;
use player::filtros::{filtro_etiqueta, FILTROS};
use std::collections::HashMap;
use twilight_model::channel::message::component::{
    ButtonStyle, Component, SelectMenu, SelectMenuOption, SelectMenuType,
};

const LOOP_KEYS: [&str; 3] = [
    "buttons.loop_off",
    "buttons.loop_track",
    "buttons.loop_queue",
];

pub fn etiqueta_bucle(idiomas: &Idiomas, lang: &str, modo: u8) -> String {
    let key = LOOP_KEYS
        .get(modo as usize)
        .copied()
        .unwrap_or(LOOP_KEYS[0]);
    idiomas.t(lang, key, &HashMap::new())
}

pub fn etiqueta_filtro(idiomas: &Idiomas, lang: &str, filtro: Option<&str>) -> String {
    match filtro {
        Some(nombre) => filtro_etiqueta(nombre)
            .map(|s| s.to_string())
            .unwrap_or_else(|| nombre.to_string()),
        None => idiomas.t(lang, "ui.filter_none", &HashMap::new()),
    }
}

pub fn filas_reproductor(
    idiomas: &Idiomas,
    lang: &str,
    estado: &InstantaneaCola,
) -> Vec<Component> {
    let empty_vars = HashMap::new();
    let pause_key = if estado.pausada {
        "buttons.resume"
    } else {
        "buttons.pause"
    };
    let pause_lbl = idiomas.t(lang, pause_key, &empty_vars);
    let loop_style = if estado.modo_repeticion != 0 {
        ButtonStyle::Success
    } else {
        ButtonStyle::Secondary
    };
    let auto_style = if estado.autoplay {
        ButtonStyle::Success
    } else {
        ButtonStyle::Secondary
    };

    let fila1 = fila(vec![
        boton(
            id::PREVIOUS,
            &idiomas.t(lang, "ui.previous", &empty_vars),
            ButtonStyle::Secondary,
            false,
        ),
        boton(id::PAUSE, &pause_lbl, ButtonStyle::Primary, false),
        boton(
            id::SKIP,
            &idiomas.t(lang, "buttons.skip", &empty_vars),
            ButtonStyle::Secondary,
            false,
        ),
        boton(
            id::STOP,
            &idiomas.t(lang, "buttons.stop", &empty_vars),
            ButtonStyle::Danger,
            false,
        ),
        boton(
            id::VOLUME,
            &idiomas.t(lang, "buttons.volume", &empty_vars),
            ButtonStyle::Secondary,
            false,
        ),
    ]);

    let auto_key = if estado.autoplay {
        "buttons.autoplay_on"
    } else {
        "buttons.autoplay_off"
    };

    let fila2 = fila(vec![
        boton(
            id::SHUFFLE,
            &idiomas.t(lang, "buttons.shuffle", &empty_vars),
            ButtonStyle::Secondary,
            estado.proximas.len() < 2,
        ),
        boton(
            id::LOOP,
            &etiqueta_bucle(idiomas, lang, estado.modo_repeticion),
            loop_style,
            false,
        ),
        boton(
            id::AUTOPLAY,
            &idiomas.t(lang, auto_key, &empty_vars),
            auto_style,
            false,
        ),
        boton(
            id::QUEUE,
            &idiomas.t(lang, "buttons.queue", &empty_vars),
            ButtonStyle::Secondary,
            false,
        ),
        boton(
            id::FAVORITE,
            &idiomas.t(lang, "buttons.favorite", &empty_vars),
            ButtonStyle::Secondary,
            false,
        ),
    ]);

    let mut opciones = vec![SelectMenuOption {
        default: estado.filtro.is_none(),
        description: None,
        emoji: None,
        label: idiomas.t(lang, "ui.filter_none", &empty_vars),
        value: "none".to_string(),
    }];
    for &nombre in FILTROS {
        let label = filtro_etiqueta(nombre).unwrap_or(nombre).to_string();
        opciones.push(SelectMenuOption {
            default: estado.filtro.as_deref() == Some(nombre),
            description: None,
            emoji: None,
            label,
            value: nombre.to_string(),
        });
    }

    let filt_ph = idiomas.t(lang, "ui.filter_placeholder", &empty_vars);
    let placeholder = format!(
        "{filt_ph}: {}",
        etiqueta_filtro(idiomas, lang, estado.filtro.as_deref())
    );

    let fila3 = fila(vec![Component::SelectMenu(SelectMenu {
        channel_types: None,
        custom_id: id::FILTER.to_string(),
        default_values: None,
        disabled: false,
        id: None,
        kind: SelectMenuType::Text,
        max_values: None,
        min_values: None,
        options: Some(opciones),
        placeholder: Some(placeholder),
        required: None,
    })]);

    let fila4 = fila(vec![boton(
        id::LYRICS,
        &idiomas.t(lang, "buttons.lyrics", &empty_vars),
        ButtonStyle::Secondary,
        false,
    )]);

    vec![fila1, fila2, fila3, fila4]
}
