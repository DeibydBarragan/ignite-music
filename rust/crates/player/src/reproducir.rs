use crate::cola::{Cola, ElementoCola};
use crate::musica::ResultadoReproducir;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ErrorReproducir {
    #[error("QUEUE_FULL")]
    ColaLlena,
    #[error("NON_NSFW")]
    NoPermitidoNonNsfw,
    #[error("EMPTY_FILTERED_PLAYLIST")]
    ListaFiltradaVacia,
    #[error("EMPTY_PLAYLIST")]
    ListaVacia,
}

pub fn encolar(
    cola: &mut Cola,
    mut elementos: Vec<ElementoCola>,
    posicion: usize,
    saltar: bool,
    max_queue_size: usize,
    canal_es_nsfw: bool,
) -> Result<ResultadoReproducir, ErrorReproducir> {
    if elementos.is_empty() {
        return Err(ErrorReproducir::ListaVacia);
    }

    if !canal_es_nsfw {
        if elementos.len() == 1 {
            if elementos[0].cancion.edad_restringida {
                return Err(ErrorReproducir::NoPermitidoNonNsfw);
            }
        } else {
            let todas_restringidas = elementos.iter().all(|e| e.cancion.edad_restringida);
            if todas_restringidas {
                return Err(ErrorReproducir::ListaFiltradaVacia);
            }
            elementos.retain(|e| !e.cancion.edad_restringida);
        }
    }

    let capacidad = cola.capacidad_restante(max_queue_size);
    if capacidad == 0 {
        return Err(ErrorReproducir::ColaLlena);
    }

    let mut truncado = false;
    if elementos.len() > capacidad {
        elementos.truncate(capacidad);
        truncado = true;
    }

    if cola.canciones.is_empty() {
        cola.canciones = elementos;
        cola.detenida = false;
        cola.pausada = false;
        return Ok(ResultadoReproducir {
            posicion: 0,
            truncado,
            empezada: true,
        });
    }

    if saltar {
        let actual = cola.canciones.remove(0);
        cola.canciones_previas.push(actual);
        for (i, elem) in elementos.into_iter().enumerate() {
            cola.canciones.insert(i, elem);
        }
        cola.pausada = false;
        return Ok(ResultadoReproducir {
            posicion: 0,
            truncado,
            empezada: true,
        });
    }

    let pos_final = if posicion == 1 {
        for (i, elem) in elementos.into_iter().enumerate() {
            cola.canciones.insert(1 + i, elem);
        }
        1
    } else {
        let start = cola.canciones.len();
        cola.canciones.extend(elementos);
        start
    };

    Ok(ResultadoReproducir {
        posicion: pos_final,
        truncado,
        empezada: false,
    })
}
