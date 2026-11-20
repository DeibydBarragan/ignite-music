use crate::cola::Cola;
use serde::{Deserialize, Serialize};

pub const FILTROS: &[&str] = &[
    "bassboost",
    "nightcore",
    "vaporwave",
    "3d",
    "karaoke",
    "echo",
    "surround",
    "tremolo",
];

pub fn filtro_etiqueta(nombre: &str) -> Option<&'static str> {
    match nombre {
        "bassboost" => Some("Bass Boost"),
        "nightcore" => Some("Nightcore"),
        "vaporwave" => Some("Vaporwave"),
        "3d" => Some("8D"),
        "karaoke" => Some("Karaoke"),
        "echo" => Some("Echo"),
        "surround" => Some("Surround"),
        "tremolo" => Some("Tremolo"),
        _ => None,
    }
}

pub fn filtro_ffmpeg_arg(nombre: &str) -> Option<&'static str> {
    match nombre {
        "3d" => Some("apulsator=hz=0.125"),
        "bassboost" => Some("bass=g=10"),
        "echo" => Some("aecho=0.8:0.9:1000:0.3"),
        "karaoke" => Some("stereotools=mlev=0.1"),
        "nightcore" => Some("asetrate=48000*1.25,aresample=48000,bass=g=5"),
        "vaporwave" => Some("asetrate=48000*0.8,aresample=48000,atempo=1.1"),
        "surround" => Some("surround"),
        "tremolo" => Some("tremolo"),
        _ => None,
    }
}

pub fn es_filtro_valido(nombre: &str) -> bool {
    filtro_ffmpeg_arg(nombre).is_some()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultadoFijarFiltro {
    pub ok: bool,
    pub cambio: bool,
    pub etiqueta: Option<&'static str>,
    pub clave_mensaje: &'static str,
}

pub fn fijar_filtro_cola(cola: &mut Cola, nombre: &str) -> ResultadoFijarFiltro {
    if nombre.is_empty() || nombre == "none" {
        let cambio = !cola.filtros.is_empty();
        if cambio {
            cola.filtros.clear();
        }
        return ResultadoFijarFiltro {
            ok: true,
            cambio,
            etiqueta: None,
            clave_mensaje: "ui.filter_none",
        };
    }

    let etiqueta = match filtro_etiqueta(nombre) {
        Some(e) => e,
        None => {
            return ResultadoFijarFiltro {
                ok: false,
                cambio: false,
                etiqueta: None,
                clave_mensaje: "errors.unknown",
            }
        }
    };

    if cola.filtros.len() == 1 && cola.filtros[0] == nombre {
        return ResultadoFijarFiltro {
            ok: true,
            cambio: false,
            etiqueta: Some(etiqueta),
            clave_mensaje: "ui.filter",
        };
    }

    cola.filtros = vec![nombre.to_string()];
    ResultadoFijarFiltro {
        ok: true,
        cambio: true,
        etiqueta: Some(etiqueta),
        clave_mensaje: "ui.filter",
    }
}
