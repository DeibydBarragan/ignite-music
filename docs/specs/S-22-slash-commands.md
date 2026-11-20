# Spec S-22: Slash commands y su registro

- Estado: implementado
- Fase: F5
- Módulo: M5
- Origen en Node (solo lectura): `src/commands/*.js` (25 comandos), `src/commands/_shared.js` (`command`, `queueAction`), `src/commands/index.js` (`loadCommands`), `src/deploy.js` (`deployCommands`, `commandBodies`), `src/handlers/interactions.js` (`route`, `onInteraction`), `src/bot.js` (IDs de comandos en `ClientReady`), `test/commands.test.js`.

## Objetivo

Definir en Rust los 25 comandos de barra con los mismos nombres, opciones, límites y localizaciones que en Node. Registrarlos en Discord cuando cambian. Enrutar cada comando y cada autocompletado a su función.

La lógica de cada comando queda en la spec de su módulo: acciones de cola (S-17), filtros (S-18), voz (S-16), panel (S-24), vistas (S-25) y permisos (S-23).

## Archivos

- Creado: `rust/crates/bot/src/comandos/mod.rs`: lista de los 25 comandos, `Contexto` (traductor del servidor y reproductor) y despacho por nombre.
- Creado: `rust/crates/bot/src/comandos/definiciones/mod.rs`, `reproduccion.rs`, `reproduccion_a.rs`, `reproduccion_b.rs` (17 comandos), `cola.rs` (6) y `ajustes.rs` (2): constructores de cada comando.
- Creado: `rust/crates/bot/src/comandos/acciones.rs`: `accion_de_cola` (equivalente a `queueAction`) y los comandos que la usan.
- Creado: `rust/crates/bot/src/comandos/reproducir.rs`: `play`, `playnext`, `reproducir_desde_interaccion` y autocompletado.
- Creado: `rust/crates/bot/src/comandos/registro.rs`: registro en Discord, marcador y IDs de comandos.
- Creado: `rust/crates/bot/src/interacciones.rs`: router de interacciones. Comandos y autocompletado van aquí. Los componentes se pasan a S-24 (`m:*`) y S-25 (`q:`, `ly:`, `s:`, `lang:set`).
- Creado: `rust/crates/bot/tests/comandos.rs` y `rust/crates/bot/tests/fixtures/comandos-node.json`.
- No tocado: `src/commands/`, `src/deploy.js`, `src/handlers/interactions.js`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada: interacción con `guild_id`. Tipos: comando de barra (nombre y opciones), autocompletado (comando, opción enfocada y texto parcial), componente y modal (S-24 y S-25).
- Salida:
  - Comando: respuesta directa, diferida (`defer`) o de actualización (`update`), con las banderas que indique cada comando.
  - Autocompletado: hasta 25 opciones con `name` y `value`.
- Errores:
  - Nombre desconocido: no responde y registra un aviso.
  - Error dentro de un comando: se registra. Si la interacción sigue viva, responde con `processing_error` en aviso efímero (S-23, `avisar`). Si venció o ya tuvo respuesta, no hace nada.
  - Autocompletado con error: responde con una lista vacía.

## Comportamiento

1. Los 25 comandos y sus opciones están en la tabla de abajo. La fuente de verdad es `commandBodies()` de Node. Su salida es la referencia de la prueba de paridad.
2. Todos los comandos son solo de servidor (`contexts: [Guild]`). No funcionan en mensajes directos.
3. Cada comando recibe un `Contexto` con el traductor del servidor (S-03) y el reproductor (S-17).
4. Localizaciones: cada comando tiene `description_localizations` de la clave `commands.<nombre>.description`. Las opciones `query` de `play` y `playnext` tienen además la localización de su descripción. Las demás descripciones son fijas, en inglés salvo `playlists` y `new-playlist`.
5. `language` requiere Gestionar servidor (`default_member_permissions` = "32").
6. La categoría (`playback`, `queue` o `settings`) solo sirve para ordenar `/help` (S-25).
7. `accion_de_cola`:
   1. Comprueba voz y cola con `revisar_voz` (S-23), con `requiere_cola` y sin unirse. Si falla, avisa.
   2. Ejecuta la acción dentro del bloqueo del servidor (S-17). Si la acción falla, avisa con su mensaje.
   3. Si la acción sale bien, avisa al reproductor del cambio (`changed`). S-24 refresca el panel.
   4. Responde con el mensaje de la acción en aviso efímero.
   `stop` no usa `accion_de_cola`. Comprueba voz, llama a `stop` (S-17) y responde con el mensaje. No avisa del cambio, igual que en Node.
8. `play` y `playnext`:
   1. Respuesta diferida y efímera.
   2. La consulta es la opción `query` sin espacios al inicio ni al final. Si queda vacía, se avisa (ver Preguntas abiertas).
   3. `reproducir_desde_interaccion(consulta, siguiente)`: `revisar_voz` con unirse y sin requerir cola. Después llama a `manager.play` (S-17) con el canal de voz, `canal_de_panel` (S-23), el miembro, la consulta y la posición (1 si es `playnext`, 0 si no).
   4. Mensaje de resultado: lista (número de temas, con nota si se recortó), canción iniciada, canción en cola siguiente (`playnext`) o canción en cola con su posición (`play`).
   5. Error `QUEUE_FULL` se avisa con `ui.queue_full` y el máximo. Otro error pasa por `explainError` (S-13). Si la clave es `errors.unknown`, se añade el detalle recortado a 200 caracteres.
9. Autocompletado de `play.query` y `playnext.query`: `choices(texto)` de `src/music/suggest.js`, sin innertube (S-15). Los videos con duración vienen de `ytsearch` (S-13 y S-14), con un límite de tiempo menor que los 3 s de Discord. Si no llegan a tiempo, se muestran solo sugerencias de texto. Hasta 25 opciones.
10. Registro al arrancar (`registro.rs`):
    1. Construye los 25 cuerpos JSON.
    2. Calcula el SHA-256 de los cuerpos junto con `CLIENT_ID` y `GUILD_ID`.
    3. Si el marcador existe y coincide, no hace nada.
    4. Si no, hace un `PUT` con la lista completa. Va a `guilds/{GUILD_ID}/commands` si `GUILD_ID` tiene valor, o a los comandos globales si no.
    5. Si el `PUT` sale bien, escribe el marcador.
    6. Error 50001 (sin acceso) o 10004 (servidor desconocido): aviso de que el bot no está en ese servidor. Otros errores se registran. En todos los casos, el arranque sigue.
11. IDs de comandos: tras el primer `Ready`, se piden los comandos (del servidor si hay `GUILD_ID`) y se guarda el mapa nombre a ID. S-25 lo usa en `/help`. Si la petición falla, se registra un aviso y `/help` muestra nombres sin mención.
12. El JSON de los comandos respeta los límites de Discord: nombre de 32 caracteres, descripción de 100, 25 opciones por comando y 25 elecciones por opción.

## Tabla de comandos

Son 25: 17 de reproducción, 6 de cola y 2 de ajustes.

| Comando | Categoría | Opciones | Lógica |
| :--- | :--- | :--- | :--- |
| `autoplay` | playback | ninguna | S-17, `toggleAutoplay` |
| `clear` | queue | ninguna | S-17, `clear` |
| `filter` | playback | `name` (texto, obligatorio; valores `none` y los 8 filtros) | S-17, `setFilter`, con los filtros de S-18 |
| `help` | settings | ninguna | S-25 |
| `jump` | queue | `position` (entero, mínimo 1, obligatorio) | S-17, `jump` |
| `language` | settings | ninguna. Requiere Gestionar servidor | S-25 |
| `loop` | playback | `mode` (texto, obligatorio; `off`, `song`, `queue`) | S-17, `setLoop` |
| `lyrics` | playback | `song` (texto, máximo 200) | S-25 |
| `move` | queue | `from` y `to` (enteros, mínimo 1, obligatorios) | S-17, `move` |
| `new-playlist` | playback | `url` (texto, máximo 1000, obligatorio); `nombre` (texto, máximo 60) | S-25 |
| `nowplaying` | playback | ninguna | S-24 |
| `pause` | playback | ninguna | S-17, `pause` |
| `play` | playback | `query` (texto, máximo 1000, obligatorio, autocompletado); `next` (booleano) | Esta spec (`reproducir.rs`) |
| `playlists` | playback | `nombre` (máximo 60); `reproducir` (máximo 500); `page` (entero, mínimo 1) | S-25 |
| `playnext` | playback | `query` (texto, máximo 1000, obligatorio, autocompletado) | Esta spec (`reproducir.rs`) |
| `previous` | playback | ninguna | S-17, `previous` |
| `queue` | queue | `page` (entero, mínimo 1) | S-25 |
| `remove` | queue | `position` (entero, mínimo 1, obligatorio) | S-17, `remove` |
| `resume` | playback | ninguna | S-17, `resume` |
| `search` | playback | `query` (texto, máximo 200, obligatorio) | S-25 |
| `seek` | playback | `time` (texto, máximo 10, obligatorio) | S-17, `seek`, con `parseTime` de S-25 |
| `shuffle` | queue | ninguna | S-17, `shuffle` |
| `skip` | playback | ninguna | S-17, `skip` |
| `stop` | playback | ninguna | S-17, `stop`, sin `accion_de_cola` |
| `volume` | playback | `level` (entero de 0 al máximo de volumen de `config.bot.maxVolume`, obligatorio) | S-17, `setVolume` |

## Tests

- Paridad: `comandos-node.json` se genera con `node` desde `src/deploy.js` (`commandBodies()`). El JSON de Rust debe ser igual tras ordenar las claves. Cubre nombres, descripciones, localizaciones, contextos, permisos, opciones, límites y elecciones.
- Límites de Discord: los mismos que la prueba `all commands serialize and respect Discord limits` de `test/commands.test.js`. Hay 25 comandos.
- Cada comando tiene categoría válida, `execute` y, donde corresponde, `autocomplete`. Es la comprobación de `test/commands.test.js`.
- Hash: el mismo cuerpo da el mismo hash. Cambiar una opción cambia el hash.
- Despacho: un nombre desconocido no responde y deja un aviso. El autocompletado devuelve como máximo 25 opciones.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- El JSON de los 25 comandos de Rust es igual al de Node. No hay diferencias.
- Con el token de prueba, el registro en el servidor de prueba pasa sin error 400.
- Un segundo arranque sin cambios no hace `PUT`.
- `/play` y `/playnext` muestran resultados al escribir.
- `/pause`, `/skip`, `/volume 50` y `/loop song` funcionan en el servidor de prueba.
- `cargo test` pasa.

## Decisiones

1. Marcador: `data/commands-rust.hash`. Node conserva `data/commands.hash`. Si Node y Rust usan el mismo `CLIENT_ID` en F7, cada uno hace un `PUT` de la lista completa, y el último en registrarse gana. Por eso los cuerpos deben ser iguales.
2. Textos fijos: las descripciones de `playlists` y `new-playlist` se quedan en español, por paridad. Pasarlas a `languages/*.json` es un cambio aparte, con la regla de F4 (e).
3. Consulta vacía en `play` y `playnext`: Rust porta el fallo igual que Node. Node responde con la descripción del campo ("Song name or link"). La corrección es una tarea aparte.
4. Opciones de respaldo (`song`, `name` y `options.data[0]`): no se portan. Discord solo envía las opciones definidas en el registro.
5. Reparto: `/search`, `/lyrics`, `/playlists`, `/new-playlist`, `/help`, `/queue` y `/language` están en S-25. `/nowplaying` está en S-24.

## Preguntas abiertas

- Ninguna en esta spec.
