# Spec S-17: Cola con bloqueo por guild

- Estado: borrador
- Fase: F4
- Módulo: M4
- Origen en Node (solo lectura): `src/music/playback.js` (`play`, `prefetchNext`, `remaining`, `queueFull`), `src/music/manager.js` (`state`, `withPlayLock`, `changed`, `publishSync`, `shutdown`), `src/music/events.js` (`bindDisTubeEvents`: `PLAY_SONG`, `FINISH_SONG`, `ADD_SONG`, `FINISH`, `DISCONNECT`, `DELETE_QUEUE`, `ERROR`), `src/music/actions.js` (controles y acciones de cola), `src/api/v1/ops/queue.js` (quitar, mover, ordenar, saltar, limpiar y deshacer), `test/manager.test.js`, `test/actions.test.js`, `test/queue-ops.test.js`.

## Objetivo

Guardar la cola de cada servidor y cambiarla solo dentro de un bloqueo por servidor. Sustituye a `withPlayLock` de Node. Incluye reproducir, encolar, quitar, mover, saltar, limpiar, deshacer y los controles de reproducción.

No incluye las fuentes (S-14), los filtros (S-18), las sesiones (S-19) ni la presencia en el canal (S-20).

## Archivos

- Crear: `rust/crates/player/src/cola.rs` (estado por servidor y bloqueo)
- Crear: `rust/crates/player/src/cola_operaciones.rs` (quitar, mover, ordenar, saltar, limpiar y deshacer)
- Crear: `rust/crates/player/src/acciones.rs` (pausa, reanudar, saltar, anterior, parar, mezclar, bucle, autoplay, volumen y búsqueda)
- Crear: `rust/crates/player/src/reproducir.rs` (capacidad, truncado, NSFW y arranque)
- Crear: `rust/crates/player/src/eventos_cola.rs` (auto-recuperación de cortes y eventos)
- Crear: `rust/crates/player/tests/cola.rs`, `rust/crates/player/tests/acciones.rs`, `rust/crates/player/tests/cola_operaciones.rs`, `rust/crates/player/tests/reproducir.rs`
- No tocar: `src/music/*.js`, `src/api/v1/ops/queue.js`, `test/manager.test.js`, `test/actions.test.js`, `test/queue-ops.test.js`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada de `reproducir(canal, miembro, consulta, posicion, saltar)`: canal de voz, miembro, consulta o URL, posición (0 para el final, 1 para a continuación) y si se salta la actual.
- Salida: `Resultado { posicion, truncado, empezada }`.
- Entrada de las operaciones: servidor y posición, o id de canción. Las posiciones empiezan en 1, porque 0 es la canción actual.
- Errores:
  - `ColaLlena`: capacidad agotada.
  - `NoHayCola` (`NO_QUEUE`, 404).
  - `PosicionInvalida` (400).
  - `DeshacerExpirado` (400): pasados 60 s.
  - `DeshacerVacio` (400).

## Comportamiento

1. Bloqueo por servidor: cada cambio de la cola (reproducir, quitar, mover, ordenar, saltar, limpiar, deshacer y controles) se ejecuta en orden dentro del bloqueo de su servidor. Si una tarea falla, la siguiente sigue.
2. La resolución de la canción (red) se hace fuera del bloqueo. El bloqueo cubre solo la parte que cambia la cola.
3. Unirse al canal de voz y resolver la canción van en paralelo. Si la resolución falla, el bot ya estaba en el canal y no hay cola, se programa la salida del canal (S-20).
4. Capacidad: `MAX_QUEUE_SIZE` (500 por defecto). La capacidad restante es el máximo menos las canciones en cola. Si no queda sitio: `ColaLlena`, y la cola no cambia.
5. Listas: se recortan a la capacidad restante, y se avisa del recorte.
6. Edad restringida: si el canal no es NSFW, una canción con esa marca da `NON_NSFW`. Una lista en la que todas tienen la marca da `EMPTY_FILTERED_PLAYLIST`.
7. Respuesta antes del audio: cuando la canción entra en la cola, se responde al usuario. Después se arranca el flujo, que puede tardar más.
8. Reproducir con `saltar`: si hay más de una canción, se salta la actual. Si la cola queda pausada, se reanuda.
9. Quitar (`removeIndex`, `removeTrack`): la posición 1 en adelante, nunca la canción actual. Por id, busca la primera canción con ese id entre las próximas. Si no existe: `TRACK_NOT_FOUND`.
10. Mover (`move`, `order`): valida que las dos posiciones estén entre 1 y la última. `order` busca el id de la canción y la mueve a la posición pedida.
11. Saltar a una posición (`jump`): valida la posición, salta, y reanuda si la cola estaba pausada.
12. Limpiar (`clear` de la API): quita todas las canciones próximas. Guarda sus metadatos con un plazo de 60 s para deshacer. Cada 30 s se borran las entradas vencidas.
13. Deshacer (`undo`): vuelve a meter al final de la cola las canciones guardadas. Reconstruye cada canción sin pedir su URL de audio, porque las URL caducan. Sin cola: `NO_ACTIVE_PLAYER`. Si el servidor no está en caché: `guild_not_found`.
14. Controles (`acciones`):
    - Pausa, reanudar y alternar: si la cola ya está en ese estado, no cambia y se avisa.
    - Saltar: sin canciones próximas y sin autoplay, no cambia y se avisa.
    - Anterior: sin canciones previas, no cambia y se avisa.
    - Parar: cierra el panel, para la cola y sale del canal.
    - Mezclar: necesita al menos 3 canciones (la actual y 2 próximas).
    - Bucle: alterna Apagado, Canción y Cola, en ese orden. Acepta también un modo explícito (`off`, `track`, `queue`).
    - Autoplay: alterna, o fija un estado (idempotente).
    - Volumen: entero de 0 a 100.
    - Búsqueda: no es válida en directos ni en canciones sin duración. La posición va de 0 a la duración.
    - Quitar, mover, saltar y limpiar en los botones de Discord: mismas validaciones. El botón de limpiar no guarda para deshacer (ver Preguntas).
    - `safely`: convierte los errores de DisTube que significan "no hay nada que hacer" (PAUSED, RESUMED, NO_PREVIOUS, NO_UP_NEXT, NO_RELATED, NO_SONG_POSITION) en avisos. Los demás errores se relanzan.
15. Auto-recuperación de cortes (`FINISH_SONG`): se aplica si la canción dura más de 30 s, se reprodujeron al menos 5 s, el corte llegó antes de los últimos 12 s, y es el primer corte de esa canción. Entonces se reanuda 2 s antes del corte, con el bucle de canción activado de forma temporal, y se avisa en el canal.
16. Eventos:
    - Al empezar una canción: se envía el panel, arranca el refresco, se precarga la siguiente, se guarda la sesión, se publica la sincronización y se registra en el historial.
    - Al añadir canciones: `changed`.
    - Al terminar la cola: se cierra el panel y se programa la salida (S-20).
    - Al desconectarse del canal de voz: se libera el receptor de voz.
    - Al borrarse la cola: se limpian el temporizador de salida y el panel.
17. Apagado: guarda todas las colas, marca el apagado, para los temporizadores y sale de cada canal.

## Tests

- Unitarios de `test/manager.test.js`:
  - `reproducir` une al canal mientras resuelve.
  - Las listas se truncan a la capacidad, y una cola llena se rechaza.
  - Los errores de resolución y de unión se propagan.
  - Se precarga la siguiente canción.
  - Las llamadas simultáneas a `/play` se añaden una tras otra, sin dos colas.
  - Corte prematuro: auto-recuperación con seek y orden de la cola.
  - Corte cerca del final: no hay auto-recuperación.
- Unitarios de `test/actions.test.js`: bucle, volumen, búsqueda, quitar, mover, saltar, limpiar, mezclar (mínimo de canciones) y `safely`.
- Unitarios de `test/queue-ops.test.js`: quitar por índice bajo bloqueo; quitar por id nunca quita la actual; mover por id; limpiar y deshacer con expiración a 60 s; llamadas concurrentes serializadas; sin cola, `NO_QUEUE`.
- Propios de Rust: el bloqueo serializa dos tareas a la vez; una tarea que falla no bloquea a la siguiente; la resolución no ocurre dentro del bloqueo.
- Paridad con Node: las posiciones y los resultados de limpiar, deshacer y mover de los tests anteriores dan lo mismo.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Dos `reproducir` simultáneos en un servidor sin cola producen una sola cola.
- Los tests de `manager.test.js`, `actions.test.js` y `queue-ops.test.js` tienen equivalente en Rust y dan el mismo resultado.
- Deshacer pasados 60 s devuelve `UNDO_EXPIRED`.
- `cargo test` pasa sin Discord.
- Ningún archivo pasa de 250 líneas.

## Decisiones

- Textos fijos: los de `playback.js`, `actions.js` y `events.js` salen de `languages/*.json` (S-03), con claves nuevas en las 23 lenguas. Node no cambia en F4. Un servidor en otro idioma verá el texto traducido, no el texto fijo.

## Preguntas abiertas

- Dos caminos para limpiar. El botón (`actions.clear`) no guarda para deshacer, y la API (`queue.clear`) sí. Propuesta: que los dos guarden para deshacer.
- Auto-recuperación. Node usa `queue._beginTime`, un campo privado de DisTube. Propuesta: en Rust, `seek` después de arrancar (S-16).
- Bloqueo. Propuesta: un `Mutex` por servidor que solo cubra el cambio de la cola. Así no se mantiene mientras se espera la red.
