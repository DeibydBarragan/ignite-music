//! Definiciones de comandos de reproducción (17 comandos, Spec S-22).

use super::reproduccion_a;
use super::reproduccion_b;
use super::CuerpoComando;
use nucleo::Idiomas;

pub fn definiciones(idiomas: &Idiomas, max_volume: u32) -> Vec<CuerpoComando> {
    let mut cmds = Vec::with_capacity(17);
    cmds.extend(reproduccion_a::definiciones(idiomas));
    cmds.extend(reproduccion_b::definiciones(idiomas, max_volume));
    cmds
}
