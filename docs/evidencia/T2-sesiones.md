# T2: sesiones en Rust frente a Node

Fecha: 2026-11-19. Rama: feature/rust-migration (sin commit, sin push).

## Hallazgos de la auditoría

Los cuatro puntos del revisor se comprobaron contra el código.

El punto 1 era solo parcialmente cierto. `AlmacenJson::set`, `delete` y `flush` llaman a `initialize` por dentro (rust/crates/nucleo/src/almacen.rs:93, 104, 124), así que un primer guardado sí carga el disco antes de escribir. El fallo real estaba en la restauración. `guild_ids`, `cargar` y `get` leen la caché sin inicializarla, y ningún código del bot llamaba a `initialize` antes del evento ready. Por lectura de código, la restauración veía una caché vacía y no restauraba nada. No ejecuté esa prueba contra la versión anterior.

El punto 2 era cierto. Rust solo aceptaba `youtube` y `spotify` (sesiones.rs:78). Node también guarda `direct` y `generic` (manager.js:40).

El punto 3 era cierto. Un volumen 0 se restauraba como 80 (versión anterior de sesiones_restaurar.rs).

La restauración no reproducía nada. `arrancar_reproduccion` solo unía el bot al canal. Eso también queda corregido.

## Tabla Node frente a Rust

| Tema | Node (referencia) | Rust antes | Rust después |
|---|---|---|---|
| Plugins guardados | youtube, spotify, direct, generic (src/music/manager.js:40) | youtube, spotify | youtube, spotify, direct, generic (rust/crates/player/src/sesiones.rs:78) |
| Inicialización del almacén | `store.initialize()` al crear el store (src/music/session.js) | No se llamaba antes de leer | `initialize` al arrancar (rust/crates/bot/src/main.rs:100) y al inicio de la restauración (rust/crates/player/src/sesiones_restaurar.rs:44) |
| Volumen 0 | `data.volume ?? defaultVolume` (src/music/sessions-restore.js:41) | 0 se cambiaba a 80 | `cola.volumen = data.volume` (sesiones_restaurar.rs:110) |
| Arranque de la pista | `queue.play()` (sessions-restore.js:49) | Solo unión al canal | `iniciar_pista_desde_con_settings`, la misma primitiva de /play (rust/crates/bot/src/restauracion.rs:114; /play en rust/crates/bot/src/comandos/reproducir.rs:193) |
| Espera a la caché | Obtiene el canal con `fetch` (sessions-restore.js:29-30) | Restauraba en ready sin esperar (cliente.rs:128) | Espera hasta 30 s a que estén los servidores guardados (restauracion.rs:47, sesiones_restaurar.rs:46) |
| Sesión vacía | Ignorada (src/music/session.js, `load`) | Ignorada (sesiones.rs:173-177) | Sin cambio |
| Sesión vencida (30 min) | `MAX_AGE_MS` (session.js:9) | `MAX_AGE_MS` (sesiones.rs:179) | Sin cambio |
| Sin oyentes | Cuenta humanos; los bots cuentan solo con `allowBotInput` (src/music/presence.js:33) | Excluye todos los bots (restauracion.rs:76) | Sin cambio (diferencia abierta) |
| Canal joinable | `isVoiceBased() && joinable` (sessions-restore.js:31) | Solo comprueba el tipo de canal | Sin cambio (diferencia abierta) |
| Guardado periódico | Cada 15 s (manager.js:24, 67), al cambiar la cola (events.js:100, manager.js:137) y en fallo (bot.js:119) | Solo al apagar (rust/crates/bot/src/apagado.rs:81) | Sin cambio (diferencia abierta) |
| Formato del archivo | `data/sessions/<guild>.json`, campos camelCase | Igual (serde camelCase en sesiones.rs) | Igual; fixture Node nuevo en `rust/crates/player/tests/data/sesiones/guild_mixta.json` |

## Pruebas nuevas y existentes

Pruebas nuevas en `rust/crates/player/tests/sesiones.rs`:

- `test_restaura_sesion_escrita_por_node_con_todos_los_tipos`: carga el fixture `guild_mixta.json` con las cuatro fuentes, volumen 0, pausa, filtros y posición 40 s. Comprueba el orden de las pistas, el volumen, la pausa, los filtros, el canal y el seek.
- `test_sin_oyentes_no_arranca_audio`: sin oyentes no se arranca audio.
- `test_sesion_vacia_o_vencida_no_arranca_audio`: sesión vacía o vencida no arranca audio.
- `test_restaurar_sin_initialize_lee_el_disco`: la restauración lee los archivos del disco sin llamar antes a `initialize`.
- `test_primer_guardado_conserva_sesiones_del_disco`: el primer guardado no pierde las sesiones que ya estaban en disco.

Pruebas existentes que siguen pasando, incluida la de round-trip:

- `test_sesiones_paridad_escritura_rust_leida_por_node`: Rust escribe y Node lee con `scripts/sesiones-paridad-ref.js` (Node v22.18.0). Node no se modificó.
- `test_sesiones_paridad_leer_archivos_de_node`
- `test_sesiones_roundtrip_sin_red`, `test_sesiones_viejas_o_vacias_se_ignoran`, `test_cancion_plugin_desconocido_se_descarta`, `test_restaurar_sesiones_logica_y_seek`

## Gates

| Comando | Resultado |
|---|---|
| `cargo fmt --check` (en rust/) | Exit 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | Exit 0 |
| `cargo test --workspace --locked` | Exit 0. Antes: 321 pasan, 2 ignoradas. Después: 326 pasan, 2 ignoradas |
| `cargo build --release --locked` | Exit 0 |
| `npm test` (raíz) | 234 pruebas: 227 pasan, 0 fallan, 7 omitidas. Sin cambio |
| `rust/scripts/check-source-size.ps1` | OK, ningún módulo supera 250 líneas |

## Archivos tocados

- `rust/crates/player/src/sesiones.rs`: una línea, la lista de plugins.
- `rust/crates/player/src/sesiones_restaurar.rs`: inicialización, espera de caché, volumen y arranque de pista.
- `rust/crates/bot/src/restauracion.rs`: espera de caché y arranque con la misma primitiva que /play.
- `rust/crates/bot/src/main.rs`: `initialize` al arrancar y paso de ajustes de yt-dlp.
- `rust/crates/player/tests/sesiones.rs` y `rust/crates/player/tests/data/sesiones/guild_mixta.json`: pruebas y fixture.

Node no se modificó.

## Sin verificar

- La unión real a voz de Discord, la caché con gateway real y la reproducción audible con yt-dlp no se probaron. Hacen falta un servidor y un token.
- `iniciar_pista` aplica solo el primer filtro (`cola.filtros.first()`). Node aplica todos. Es el comportamiento existente de /play y no lo cambié.
- El guardado periódico y al cambiar la cola de Node no existe en Rust. Una caída del proceso pierde la sesión en curso. Lo dejé fuera del alcance de T2; haría falta un item propio.
- Diferencias abiertas en oyentes (bots con `allowBotInput`) y en `joinable`. Están en la tabla.
- Si el archivo de sesión no tiene `volume`, Rust lo lee como 0 (serde por defecto). Node siempre escribe el campo, así que no pasa con sesiones de Node.
