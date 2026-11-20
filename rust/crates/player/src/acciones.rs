use crate::cola::{Cola, ModoRepeticion};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultadoAccion {
    pub ok: bool,
    pub clave: &'static str,
    pub detalle: Option<String>,
}

impl ResultadoAccion {
    pub fn si(clave: &'static str) -> Self {
        Self {
            ok: true,
            clave,
            detalle: None,
        }
    }

    pub fn si_con_detalle(clave: &'static str, detalle: impl Into<String>) -> Self {
        Self {
            ok: true,
            clave,
            detalle: Some(detalle.into()),
        }
    }

    pub fn no(clave: &'static str) -> Self {
        Self {
            ok: false,
            clave,
            detalle: None,
        }
    }
}

pub fn pausar(cola: &mut Cola) -> ResultadoAccion {
    if cola.pausada {
        return ResultadoAccion::no("ui.already_paused");
    }
    cola.pausada = true;
    ResultadoAccion::si("buttonhandler.music_paused")
}

pub fn reanudar(cola: &mut Cola) -> ResultadoAccion {
    if !cola.pausada {
        return ResultadoAccion::no("ui.not_paused");
    }
    cola.pausada = false;
    ResultadoAccion::si("buttonhandler.music_resumed")
}

pub fn alternar_pausa(cola: &mut Cola) -> ResultadoAccion {
    if cola.pausada {
        reanudar(cola)
    } else {
        pausar(cola)
    }
}

pub fn saltar(cola: &mut Cola) -> ResultadoAccion {
    if cola.canciones.len() <= 1 && !cola.autoplay {
        return ResultadoAccion::no("buttonhandler.no_songs_to_skip");
    }
    if cola.canciones.is_empty() {
        return ResultadoAccion::no("buttonhandler.no_songs_to_skip");
    }
    let actual = cola.canciones.remove(0);
    let nombre = actual.cancion.nombre.clone();
    cola.canciones_previas.push(actual);
    cola.pausada = false;
    ResultadoAccion::si_con_detalle("buttonhandler.song_skipped_title", nombre)
}

pub fn anterior(cola: &mut Cola) -> ResultadoAccion {
    if cola.canciones_previas.is_empty() {
        return ResultadoAccion::no("buttonhandler.no_previous_song");
    }
    let prev = cola.canciones_previas.pop().unwrap();
    cola.canciones.insert(0, prev);
    cola.pausada = false;
    ResultadoAccion::si("buttonhandler.moved_to_previous")
}

pub fn parar(cola: &mut Cola) -> ResultadoAccion {
    cola.detenida = true;
    cola.pausada = false;
    cola.canciones.clear();
    cola.canciones_previas.clear();
    ResultadoAccion::si("buttonhandler.music_stopped_title")
}

pub fn mezclar(cola: &mut Cola) -> ResultadoAccion {
    if cola.canciones.len() < 3 {
        return ResultadoAccion::no("buttonhandler.minimum_songs_shuffle");
    }
    let mut rng = rand::thread_rng();
    cola.canciones[1..].shuffle(&mut rng);
    ResultadoAccion::si_con_detalle(
        "buttonhandler.songs_shuffled",
        (cola.canciones.len() - 1).to_string(),
    )
}

pub fn fijar_bucle(cola: &mut Cola, modo: Option<ModoRepeticion>) -> ResultadoAccion {
    let siguiente = modo.unwrap_or_else(|| cola.modo_repeticion.siguiente());
    cola.modo_repeticion = siguiente;
    let clave = match siguiente {
        ModoRepeticion::Apagado => "buttonhandler.loop_mode_off",
        ModoRepeticion::Cancion => "buttonhandler.loop_mode_track",
        ModoRepeticion::Cola => "buttonhandler.loop_mode_queue",
    };
    ResultadoAccion::si(clave)
}

pub fn alternar_autoplay(cola: &mut Cola) -> ResultadoAccion {
    cola.autoplay = !cola.autoplay;
    let clave = if cola.autoplay {
        "buttons.autoplay_on"
    } else {
        "buttons.autoplay_off"
    };
    ResultadoAccion::si(clave)
}

pub fn fijar_autoplay(cola: &mut Cola, activo: bool) -> ResultadoAccion {
    cola.autoplay = activo;
    let clave = if cola.autoplay {
        "buttons.autoplay_on"
    } else {
        "buttons.autoplay_off"
    };
    ResultadoAccion::si(clave)
}

pub fn fijar_volumen(cola: &mut Cola, volumen: i64, max_volumen: u8) -> ResultadoAccion {
    if volumen < 0 || volumen > max_volumen as i64 {
        return ResultadoAccion::no("modalhandler.invalid_volume");
    }
    cola.volumen = volumen as u8;
    ResultadoAccion::si_con_detalle("modalhandler.volume_changed_desc", volumen.to_string())
}

pub fn fijar_filtro(cola: &mut Cola, nombre: &str) -> ResultadoAccion {
    if nombre.is_empty() || nombre == "none" {
        cola.filtros.clear();
        return ResultadoAccion::si("ui.filter_none");
    }
    cola.filtros = vec![nombre.to_string()];
    ResultadoAccion::si_con_detalle("ui.filter", nombre)
}

pub fn buscar(cola: &mut Cola, segundos: u64) -> ResultadoAccion {
    let song = match cola.canciones.first() {
        Some(s) => s,
        None => return ResultadoAccion::no("buttonhandler.no_songs_in_queue"),
    };
    if song.cancion.es_directo || song.cancion.duracion == 0 {
        return ResultadoAccion::no("ui.seek_live");
    }
    if segundos >= song.cancion.duracion {
        return ResultadoAccion::no("ui.seek_invalid");
    }
    cola.posicion_segundos = segundos;
    ResultadoAccion::si_con_detalle("ui.seeked", segundos.to_string())
}

pub fn aplicar_control(
    cola: &mut Cola,
    accion: &str,
    valor: Option<&str>,
    max_volumen: u8,
) -> Option<ResultadoAccion> {
    match accion {
        "pause" => Some(pausar(cola)),
        "resume" => Some(reanudar(cola)),
        "toggle" => Some(alternar_pausa(cola)),
        "skip" => Some(saltar(cola)),
        "previous" => Some(anterior(cola)),
        "stop" => Some(parar(cola)),
        "shuffle" => Some(mezclar(cola)),
        "autoplay" => Some(alternar_autoplay(cola)),
        "filter" => Some(fijar_filtro(cola, valor.unwrap_or("none"))),
        "volume" => {
            let v = valor.and_then(|s| s.parse::<i64>().ok())?;
            Some(fijar_volumen(cola, v, max_volumen))
        }
        "seek" => {
            let s = valor.and_then(|s| s.parse::<u64>().ok())?;
            Some(buscar(cola, s))
        }
        "loop" => {
            let mode = valor.and_then(ModoRepeticion::desde_str);
            Some(fijar_bucle(cola, mode))
        }
        _ => None,
    }
}
