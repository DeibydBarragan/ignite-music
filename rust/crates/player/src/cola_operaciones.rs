use crate::cola::{Cola, ElementoCola, GestorColas};
use thiserror::Error;

pub const UNDO_TTL_MS: u64 = 60_000;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ErrorOperacionCola {
    #[error("NO_QUEUE")]
    NoHayCola,
    #[error("INVALID_INDEX")]
    PosicionInvalida,
    #[error("TRACK_NOT_FOUND")]
    CancionNoEncontrada,
    #[error("UNDO_EMPTY")]
    DeshacerVacio,
    #[error("UNDO_EXPIRED")]
    DeshacerExpirado,
}

pub fn quitar_por_indice(
    cola: &mut Cola,
    index: usize,
) -> Result<ElementoCola, ErrorOperacionCola> {
    if index < 1 || index >= cola.canciones.len() {
        return Err(ErrorOperacionCola::PosicionInvalida);
    }
    Ok(cola.canciones.remove(index))
}

pub fn quitar_por_track_id(
    cola: &mut Cola,
    track_id: &str,
) -> Result<ElementoCola, ErrorOperacionCola> {
    let idx = cola
        .canciones
        .iter()
        .enumerate()
        .position(|(i, s)| i > 0 && s.track_id == track_id);

    match idx {
        Some(i) => Ok(cola.canciones.remove(i)),
        None => Err(ErrorOperacionCola::CancionNoEncontrada),
    }
}

pub fn mover(cola: &mut Cola, from: usize, to: usize) -> Result<String, ErrorOperacionCola> {
    let last = cola.canciones.len().saturating_sub(1);
    if from < 1 || from > last || to < 1 || to > last {
        return Err(ErrorOperacionCola::PosicionInvalida);
    }
    let song = cola.canciones.remove(from);
    let title = song.cancion.nombre.clone();
    cola.canciones.insert(to, song);
    Ok(title)
}

pub fn ordenar(cola: &mut Cola, track_id: &str, to: usize) -> Result<String, ErrorOperacionCola> {
    let from = cola
        .canciones
        .iter()
        .enumerate()
        .position(|(i, s)| i > 0 && s.track_id == track_id)
        .ok_or(ErrorOperacionCola::CancionNoEncontrada)?;

    mover(cola, from, to)
}

pub fn saltar_a(cola: &mut Cola, position: usize) -> Result<ElementoCola, ErrorOperacionCola> {
    if position < 1 || position >= cola.canciones.len() {
        return Err(ErrorOperacionCola::PosicionInvalida);
    }
    let removed: Vec<ElementoCola> = cola.canciones.drain(0..position).collect();
    cola.canciones_previas.extend(removed);
    cola.pausada = false;
    cola.canciones
        .first()
        .cloned()
        .ok_or(ErrorOperacionCola::PosicionInvalida)
}

pub async fn limpiar(
    cola: &mut Cola,
    ahora_ms: u64,
    gestor: &GestorColas,
) -> Result<usize, ErrorOperacionCola> {
    let count = cola.canciones.len().saturating_sub(1);
    if count > 0 {
        let removed: Vec<ElementoCola> = cola.canciones.drain(1..).collect();
        gestor
            .guardar_deshacer(&cola.guild_id, removed, ahora_ms + UNDO_TTL_MS)
            .await;
    }
    Ok(count)
}

pub async fn deshacer(
    cola: &mut Cola,
    ahora_ms: u64,
    gestor: &GestorColas,
) -> Result<usize, ErrorOperacionCola> {
    let entrada = gestor
        .obtener_deshacer(&cola.guild_id)
        .await
        .ok_or(ErrorOperacionCola::DeshacerVacio)?;

    if ahora_ms > entrada.expira_en {
        return Err(ErrorOperacionCola::DeshacerExpirado);
    }
    if entrada.canciones.is_empty() {
        return Err(ErrorOperacionCola::DeshacerVacio);
    }

    let count = entrada.canciones.len();
    cola.canciones.extend(entrada.canciones);
    Ok(count)
}
