//! Definiciones de comandos de cola (6 comandos, Spec S-22).

use super::{comando_base, opcion_integer, CuerpoComando};
use nucleo::Idiomas;

pub fn definiciones(idiomas: &Idiomas) -> Vec<CuerpoComando> {
    let mut cmds = Vec::with_capacity(6);

    // 1. clear
    cmds.push(comando_base(
        "clear",
        "Remove every upcoming song from the queue",
        idiomas,
    ));

    // 2. jump
    let mut jump = comando_base("jump", "Skip to a song in the queue", idiomas);
    jump.options.push(opcion_integer(
        "position",
        "Position in the queue",
        true,
        Some(1),
        None,
    ));
    cmds.push(jump);

    // 3. move
    let mut move_cmd = comando_base(
        "move",
        "Move a song to another position in the queue",
        idiomas,
    );
    move_cmd.options.push(opcion_integer(
        "from",
        "Current position",
        true,
        Some(1),
        None,
    ));
    move_cmd
        .options
        .push(opcion_integer("to", "New position", true, Some(1), None));
    cmds.push(move_cmd);

    // 4. queue
    let mut queue = comando_base("queue", "Show the songs in the queue", idiomas);
    queue
        .options
        .push(opcion_integer("page", "Page number", false, Some(1), None));
    cmds.push(queue);

    // 5. remove
    let mut remove = comando_base("remove", "Remove a song from the queue", idiomas);
    remove.options.push(opcion_integer(
        "position",
        "Position in the queue",
        true,
        Some(1),
        None,
    ));
    cmds.push(remove);

    // 6. shuffle
    cmds.push(comando_base("shuffle", "Shuffle the queue", idiomas));

    cmds
}
