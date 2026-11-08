# Spec S-20: Presencia y salida automática

- Estado: borrador
- Fase: F4
- Módulo: M4
- Origen en Node (solo lectura): `src/music/presence.js` (`scheduleLeave`, `clearLeave`, `listenerCount`, `handleVoiceStateUpdate`), `src/music/manager.js` (`listenerCount`, `handleVoiceStateUpdate`, `shutdown`), `src/music/events.js` (`FINISH`, `DELETE_QUEUE`), `test/manager.test.js` (grupo de canal vacío).

## Objetivo

Pausar la música cuando el canal se queda sin personas, reanudarla cuando alguien vuelve, y salir del canal tras un tiempo de inactividad. Salir también cuando la cola termina.

No incluye la cola (S-17) ni las sesiones (S-19).

## Archivos

- Crear: `rust/crates/player/src/presencia.rs` (temporizadores y conteo de personas)
- Crear: `rust/crates/player/tests/presencia.rs`
- No tocar: `src/music/presence.js`, `test/manager.test.js`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada de `actualizar_voz(antes, despues)`: estados de voz de Discord (GUILD_VOICE_STATES).
- Entrada de `contar_personas(servidor, canal)`: número de personas en el canal del bot.
- Configuración:
  - `LEAVE_ON_EMPTY`: segundos. Por defecto 60. Con 0 se desactiva.
  - `LEAVE_ON_FINISH`: segundos. Por defecto 120. Con 0 se desactiva.
- Salida: efectos sobre la cola (pausar y reanudar) y sobre el canal (salir).

## Comportamiento

1. Conteo de personas: son los usuarios del canal del bot que no son bots. Hay una excepción: el bot de sonido cuenta si `VOICE_ALLOW_BOT_INPUT` está activo y `IGNITE_SOUND_BOT_ID` está definido. Se usa el estado de voz, no los miembros, para no necesitar el intent privilegiado.
2. Al terminar la cola (`FINISH`): si `LEAVE_ON_FINISH` es mayor que 0, se programa la salida para dentro de ese tiempo. Si al vencer no hay cola, el bot sale del canal.
3. Al empezar una canción o al crearse una cola: se cancela la salida programada.
4. Canal que se queda vacío:
   a. Si `LEAVE_ON_EMPTY` es mayor que 0 y no hay temporizador de vacío, se arma el temporizador.
   b. Si la cola no está pausada, se pausa, se marca como "pausada por vacío" y se refresca el panel.
5. Al vencer el temporizador de vacío, si sigue sin personas:
   a. Si hay cola, se cierra el panel con el aviso `ui.left_empty` y se para la cola.
   b. El bot sale del canal, con el motivo `inactivity_empty`.
6. Si alguien vuelve al canal antes de que venza: se cancela el temporizador. Si la cola estaba pausada por vacío, se reanuda. Se refresca el panel.
7. El temporizador se arma antes de cualquier `await`, para que dos actualizaciones de voz seguidas no creen dos temporizadores.
8. Motivos de salida: `inactivity_finished` (la cola terminó) e `inactivity_empty` (el canal se quedó vacío). Se guardan en el estado del servidor para el panel.
9. Al apagarse: se cancelan los temporizadores de salida y de vacío, y se sale de todos los canales.

## Tests

- Unitarios de `test/manager.test.js`, grupo "empty voice channel": pausa al quedarse solo, reanuda cuando vuelve alguien, sale tras el tiempo.
- Propios de Rust:
  - Un bot de sonido no cuenta como persona, salvo con `VOICE_ALLOW_BOT_INPUT`.
  - Con `LEAVE_ON_EMPTY=0`, no pausa ni sale.
  - Con `LEAVE_ON_FINISH=0`, no programa la salida.
  - Dos actualizaciones seguidas arman un solo temporizador.
  - Volver al canal cancela la salida, y reanuda solo si el bot pausó la cola.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Con el canal vacío, la música se pausa en menos de un segundo desde la última salida (medido en el servidor de prueba).
- Con `LEAVE_ON_EMPTY` en 60, el bot sale a los 60 segundos si nadie vuelve.
- Si alguien vuelve antes, el bot no sale, y reanuda la música si la pausó él.
- `cargo test` pasa sin Discord.
- Ningún archivo pasa de 250 líneas.

## Decisiones

- Se mantiene la marca `pausedForEmpty`. Solo se reanuda la pausa que puso el bot al vaciarse el canal. Una pausa manual no se reanuda sola.
- `IGNITE_SOUND_BOT_ID` conserva su valor por defecto del código, como en Node.
- Hasta F5, los eventos de voz llegan simulados en los tests de F4: un tipo con los mismos campos que el evento de Discord. S-21 los conecta en F5 (paso 6.2).

## Preguntas abiertas

- Ninguna en esta spec.
