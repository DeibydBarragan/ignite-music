//! Definiciones de comandos de ajustes (2 comandos, Spec S-22).

use super::{comando_base, CuerpoComando};
use nucleo::Idiomas;

pub fn definiciones(idiomas: &Idiomas) -> Vec<CuerpoComando> {
    let mut cmds = Vec::with_capacity(2);

    // 1. help
    cmds.push(comando_base(
        "help",
        "Show commands and bot information",
        idiomas,
    ));

    // 2. language (requiere Gestionar servidor, bit 5 = 32)
    let mut lang = comando_base(
        "language",
        "Change the bot language for this server",
        idiomas,
    );
    lang.default_member_permissions = Some("32".to_string());
    cmds.push(lang);

    cmds
}
