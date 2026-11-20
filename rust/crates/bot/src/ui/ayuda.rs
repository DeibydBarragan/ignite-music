use crate::respuesta::PayloadRespuesta;
use crate::ui::componentes::{
    boton_enlace, contenedor, fila, separador, texto, vista_base, DEFAULT_COLOR,
};
use nucleo::formato::url_segura;
use nucleo::idioma::Idiomas;
use std::collections::HashMap;

pub struct InfoComandoAyuda {
    pub name: String,
    pub id: Option<String>,
    pub description: String,
}

pub struct StatsAyuda {
    pub servers: u64,
    pub active: u64,
    pub uptime: String,
    pub ping: u64,
}

#[derive(Default)]
pub struct EnlacesAyuda {
    pub support_server: Option<String>,
    pub website: Option<String>,
    pub invite: Option<String>,
}

pub fn vista_ayuda(
    idiomas: &Idiomas,
    lang: &str,
    commands: &[InfoComandoAyuda],
    stats: Option<&StatsAyuda>,
    enlaces: Option<&EnlacesAyuda>,
) -> PayloadRespuesta {
    let empty_vars = HashMap::new();
    let mut componentes = Vec::new();

    // 1. Título y descripción principal
    let title_line = idiomas.t(lang, "commands.help.title", &empty_vars);
    let desc_line = idiomas.t(lang, "commands.help.main_description", &empty_vars);
    componentes.push(texto(format!("### {title_line}\n{desc_line}")));
    componentes.push(separador(false));

    // 2. Comandos
    let todos_tienen_id = commands
        .iter()
        .all(|c| c.id.as_ref().map(|s| !s.is_empty()).unwrap_or(false));
    let lines: Vec<String> = commands
        .iter()
        .map(|c| {
            if todos_tienen_id {
                let id_str = c.id.as_deref().unwrap_or("0");
                format!("</{}:{}> {}", c.name, id_str, c.description)
            } else {
                format!("`/{}` {}", c.name, c.description)
            }
        })
        .collect();

    let cmd_title = idiomas.t(lang, "commands.help.commands_title", &empty_vars);
    componentes.push(texto(format!("**{cmd_title}**\n{}", lines.join("\n"))));

    // 3. Estadísticas
    if let Some(s) = stats {
        componentes.push(separador(false));
        let mut serv_vars = HashMap::new();
        serv_vars.insert("count".to_string(), s.servers.to_string());
        let serv_lbl = idiomas.t(lang, "commands.help.stats_servers", &serv_vars);

        let mut act_vars = HashMap::new();
        act_vars.insert("count".to_string(), s.active.to_string());
        let act_lbl = idiomas.t(lang, "commands.help.stats_active", &act_vars);

        let mut up_vars = HashMap::new();
        up_vars.insert("time".to_string(), s.uptime.clone());
        let up_lbl = idiomas.t(lang, "commands.help.stats_uptime", &up_vars);

        let ping_str = format!("{} ms", s.ping);
        let stat_line = format!("-# {serv_lbl} · {act_lbl} · {up_lbl} · {ping_str}");
        componentes.push(texto(stat_line));
    }

    // 4. Enlaces
    if let Some(enl) = enlaces {
        let mut botones = Vec::new();
        if let Some(ref url) = enl.support_server {
            if url_segura(url).is_some() {
                let lbl = idiomas.t(lang, "commands.help.button_support", &empty_vars);
                botones.push(boton_enlace(&lbl, url));
            }
        }
        if let Some(ref url) = enl.website {
            if url_segura(url).is_some() {
                let lbl = idiomas.t(lang, "commands.help.button_website", &empty_vars);
                botones.push(boton_enlace(&lbl, url));
            }
        }
        if let Some(ref url) = enl.invite {
            if url_segura(url).is_some() {
                let lbl = idiomas.t(lang, "ui.invite", &empty_vars);
                botones.push(boton_enlace(&lbl, url));
            }
        }
        if !botones.is_empty() {
            componentes.push(fila(botones));
        }
    }

    let c = contenedor(Some(DEFAULT_COLOR), componentes);
    vista_base(vec![c], true)
}
