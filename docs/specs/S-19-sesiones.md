# Spec S-19: Sesiones y restauración al arrancar

- Estado: borrador
- Fase: F4
- Módulo: M4
- Origen en Node (solo lectura): `src/music/session.js` (`SessionStore`: `serialize`, `save`, `load`, `buildSongs`, `remove`), `src/music/sessions-restore.js` (`saveAllSessions`, `restoreSessions`), `src/music/manager.js` (temporizador de guardado cada 15 s, `shutdown`), `test/session.test.js`.

## Objetivo

Guardar la cola de cada servidor en disco cada 15 segundos. Al arrancar, volver a poner la cola de los servidores donde sigue habiendo gente en el canal, en la misma posición.

No incluye la cola en memoria (S-17) ni la presencia en el canal (S-20).

## Archivos

- Crear: `rust/crates/player/src/sesiones.rs` (formato, guardar y cargar)
- Crear: `rust/crates/player/src/sesiones_restaurar.rs` (restauración al arrancar)
- Crear: `rust/crates/player/tests/sesiones.rs`
- Crear: `rust/crates/player/tests/data/sesiones/` (archivos de sesión escritos por Node)
- Crear: `scripts/sesiones-paridad-ref.js` (debe pasar `npm run lint`)
- No tocar: `src/music/session.js`, `src/music/sessions-restore.js`, `test/session.test.js`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada de `guardar(cola)`: la cola en memoria.
- Salida: un archivo `data/sessions/<servidor>.json` con el formato del punto 1. Se escribe con `AlmacenJson` de S-04.
- Entrada de `cargar(servidor)`: id del servidor. Salida: la sesión, o nada si no sirve.
- Errores: un fallo al guardar o al borrar se registra como aviso.

## Comportamiento

1. Formato (JSON, un archivo por servidor):
   - `voiceChannelId`, `textChannelId`.
   - `songs`: cada canción con `plugin`, `source`, `playFromSource`, `id`, `name`, `url`, `duration`, `isLive`, `thumbnail`, `uploader` (`name`, `url`), `ageRestricted`, `requesterId`, `requesterUser` (`id`, `username`, `avatar`, `bot`) y `autoplay`.
   - `position` (segundos enteros), `volume`, `repeatMode`, `autoplay`, `filters` (nombres), `paused` y `savedAt` (milisegundos).
2. Solo se guardan canciones con `plugin` y `url` conocidos. Como máximo `MAX_QUEUE_SIZE` canciones.
3. Guardado periódico: cada 15 s se guardan las colas que no están paradas y tienen canciones. Una cola sin canciones borra su sesión.
4. Al apagarse, el bot guarda todas las colas (`shutdown`) antes de salir.
5. Carga: una sesión se ignora si no tiene canciones o si `savedAt` es de hace más de 30 minutos.
6. Restauración al arrancar, por servidor de este proceso. Un proceso solo restaura los servidores que ve:
   a. Si el servidor no está en caché, se salta.
   b. Se buscan el canal de voz y el de texto. Si el canal de voz no existe, no es de voz o no se puede unir, se salta.
   c. Si no hay personas (que no sean bots) en el canal, se salta.
   d. Se reconstruyen las canciones sin pedir nada a la red. Las canciones con un plugin desconocido se descartan.
   e. Dentro del bloqueo de S-17: si ya hay cola, se salta, porque alguien empezó música antes. Si no, se crea la cola, se añaden las canciones y se fijan el volumen (por defecto `DEFAULT_VOLUME`), el modo de bucle (0, 1 o 2; otro valor se toma como 0), el autoplay y los filtros.
   f. Posición: si la canción tiene duración, la posición guardada es mayor que 0 y menor que la duración menos 2 s, se hace `seek` a esa posición al empezar.
   g. Se arranca la reproducción.
7. Al terminar el intento, si no hay cola, se borra la sesión del servidor, aunque el intento haya fallado.
8. Un error en un servidor no detiene la restauración de los demás. Se registra como aviso.
9. Al final se registra un aviso con cuántas sesiones se restauraron.

## Tests

- Unitarios de `test/session.test.js`:
  - Las colas pasan por el almacén de sesiones sin llamadas de red.
  - Las sesiones viejas o vacías se ignoran y se pueden borrar.
- Propios de Rust:
  - Una sesión escrita por Node se carga sin cambios.
  - Una canción con plugin desconocido se descarta.
  - Restauración con un canal sin personas: se salta y se borra la sesión.
  - Restauración con cola ya existente: se salta.
  - El seek se aplica solo si la posición cabe en la canción.
- Paridad con Node: `scripts/sesiones-paridad-ref.js` escribe sesiones con `SessionStore` de Node en `tests/data/sesiones/`. El test de Rust las lee y compara los campos. Después, en sentido contrario, Rust escribe y Node lee.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Una sesión escrita por Node se restaura en Rust con la misma canción, posición y volumen.
- Un reinicio con gente en el canal restaura la cola. Sin gente, no.
- Una sesión de más de 30 minutos no se restaura.
- `cargo test` pasa sin Discord.
- Ningún archivo pasa de 250 líneas.

## Preguntas abiertas

- Carpeta compartida en F7. Si Node y Rust usan la misma carpeta `data/`, los dos procesos restauran las mismas sesiones y la música suena dos veces. Propuesta: en F7, cada uno con su propio `DATA_DIR`.
- Posición. Node usa `_beginTime`, un campo privado de DisTube. Propuesta: `seek` de songbird (S-16) después de arrancar.
- Formato. Propuesta: el mismo JSON de Node, para leer las sesiones de Node durante F7.
- Después de F8, ¿quién restaura las sesiones que hoy restaura Node? Propuesta: Rust, cuando tenga el token del bot.
