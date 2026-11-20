use crate::cola::{Cola, ModoRepeticion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccionRecuperacion {
    pub seek_target: u64,
    pub segundos_reproducidos: u64,
    pub cancion_nombre: String,
}

pub fn comprobar_corte_prematuro(
    cola: &mut Cola,
    segundos_reproducidos: u64,
) -> Option<AccionRecuperacion> {
    if cola.detenida || cola.canciones.is_empty() {
        return None;
    }
    let song = cola.canciones.first_mut()?;
    let duracion = song.cancion.duracion;

    let era_prematuro = duracion > 30
        && segundos_reproducidos >= 5
        && segundos_reproducidos < duracion.saturating_sub(12);

    if era_prematuro && song.cut_retries < 1 {
        song.cut_retries += 1;
        let seek_target = segundos_reproducidos.saturating_sub(2);
        song.original_repeat_mode = Some(cola.modo_repeticion);
        cola.modo_repeticion = ModoRepeticion::Cancion;
        cola.posicion_segundos = seek_target;

        Some(AccionRecuperacion {
            seek_target,
            segundos_reproducidos,
            cancion_nombre: song.cancion.nombre.clone(),
        })
    } else {
        None
    }
}

pub fn al_iniciar_cancion(cola: &mut Cola) {
    if let Some(song) = cola.canciones.first_mut() {
        if let Some(orig_mode) = song.original_repeat_mode.take() {
            cola.modo_repeticion = orig_mode;
        }
    }
}

pub fn al_terminar_cancion(cola: &mut Cola) -> Option<AccionRecuperacion> {
    let segs = cola.posicion_segundos;
    comprobar_corte_prematuro(cola, segs)
}

pub fn mensaje_aviso_corte(cancion_nombre: &str, segundos: u64, es_espanol: bool) -> String {
    if es_espanol {
        format!(
            "⚠️ La reproducción de **{}** se interrumpió a los {}s. Reanudando automáticamente...",
            cancion_nombre, segundos
        )
    } else {
        format!(
            "⚠️ Playback of **{}** was interrupted at {}s. Resuming automatically...",
            cancion_nombre, segundos
        )
    }
}
