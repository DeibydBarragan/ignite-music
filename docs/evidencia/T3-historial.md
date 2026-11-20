# T3: historial en cada arranque de pista

## Qué hace Node

`src/music/events.js:85-102` llama a `history.recordPlay` en `PLAY_SONG`. DisTube emite ese evento en cada arranque nuevo: primera pista, avance, autoplay y restauración (`sessions-restore.js` llama `queue.play()`). No lo emite en seek ni en cambio de filtro, porque `node_modules/distube/dist/index.js:1117` y `:591` llaman `play(false)`.

Node guarda usuario, título, artista, duración, fuente y `played_at`. No guarda canal de voz. No guarda `startup_latency_ms`; ese valor solo va al log.

## Qué cambia en Rust

El evento `"play"` del driver ahora significa arranque nuevo. Seek y filtro emiten `"reinicio"`. Solo el listener de `"play"` registra historial, una vez por arranque.

- `player/src/reproduccion.rs`: nueva `reiniciar_pista_desde_con_settings`. Los arranques existentes pasan `reinicio = false`.
- `player/src/reproduccion/efectos.rs`: seek y filtro llaman a la función de reinicio.
- `player/src/driver_reproduccion.rs`: campo `reinicio` en `OpcionesReproduccion` (por defecto `false`). `registrar_pista_con_motivo` elige `"play"` o `"reinicio"`.
- `bot/src/api/eventos_reproduccion.rs`: trait `RegistroHistorial` para inyectar el registrador. El listener llama al registrador en `"play"` con los datos de la cola. Un error del registrador se registra en log y no detiene nada. Usa el mismo `GestorDevMode` que POST /play.
- `bot/src/api/eventos_reproduccion_pruebas.rs` y `eventos_reproduccion_pruebas_origenes.rs` (nuevos): pruebas del listener. Se separaron para cumplir el límite de 250 líneas.
- `bot/src/api/ciclo_vida.rs`: el listener se conecta después de crear el contexto, para compartir el gestor de dev mode.
- `bot/src/api/proveedor_reproduccion.rs`: se quita el registro de POST /play (antes registraba el primer elemento aunque solo quedara en cola).

Sin cambios: `comandos/reproducir.rs` y `datos/historial.rs`. La ruta slash ya llama a `iniciar_pista_con_settings`, así que el listener la cubre. `restauracion.rs` también cubierto, igual que en Node.

Diferencias con Node que quedan:

- Node revisa `song.metadata.devMode`. Rust no tiene ese campo; solo revisa el dev mode del solicitante.
- POST /play ya no registra pistas en cola que no arrancan. Node tampoco.
- Límite conocido: el listener lee la cabecera de la cola al procesar el evento, no al arrancar. Si la cola cambia antes de que el listener corra, registra la pista equivocada. `shortcut: el evento "play" no lleva el título de la canción, mejorar pasando el título desde el arranque`.

## Pruebas

Pruebas del listener, con un registrador falso (`RegistradorFalso`) y el driver real de Songbird sin Discord:

- (a) `origen_comando_registra_una_vez`: arranque por `iniciar_pista_con_settings`, la ruta del slash. Una llamada.
- (a) `origen_avance_registra_una_vez_por_pista`: tras un salto (`detener_y_avanzar`), la siguiente pista se registra una vez. Dos llamadas en total, títulos A y B.
- (a) `origen_restauracion_registra_una_vez`: arranque por `iniciar_pista_desde_con_settings` con posición, la ruta de `restauracion.rs`. Una llamada.
- (b) `post_play_no_registra_dos_veces`: el arranque que hace POST /play llega por `iniciar_pista_con_settings`. Una llamada en total, porque el handler ya no registra.
- (c) `fallo_de_historial_no_detiene_la_reproduccion`: el registrador devuelve error. El listener no propaga el error. La pista siguiente arranca (`active_track` existe) y también se registra.
- `player/tests/reinicio_historial.rs::seek_y_filtro_no_emiten_arranque_para_historial`: filtro y seek emiten `reinicio`, no `play`. Un arranque posterior vuelve a `play`.
- `bot/src/api/eventos_reproduccion_pruebas.rs::entrada_marca_modo_dev_del_solicitante`.

Origen no ejercitado: autoplay. Su rama llama a `encolar_relacionado`, que necesita yt-dlp y red. Sin eso, la cola queda vacía y no arranca nada. Queda sin prueba.

Otros puntos sin prueba: POST /play como handler HTTP (la prueba usa el mismo arranque que el handler, sin la resolución de yt-dlp). Discord real y Supabase real. Restauración real desde `restauracion.rs`, solo se probó la llamada que hace.

## Comandos y conteos

- `cargo fmt --all -- --check`: exit 0.
- `cargo clippy --all-targets --locked -- -D warnings`: exit 0.
- `cargo test --workspace --locked`: 335 pasan, 0 fallan.
- `cargo build --release --locked`: exit 0.
- `npm test` en la raíz: 234 pruebas, 227 pasan, 0 fallan, 7 omitidas.

Líneas de los archivos de producción tocados: `eventos_reproduccion.rs` 229, `ciclo_vida.rs` 185, `proveedor_reproduccion.rs` 206, `reproduccion.rs` 184, `efectos.rs` 118, `driver_reproduccion.rs` 141. Pruebas: `eventos_reproduccion_pruebas.rs` 155, `eventos_reproduccion_pruebas_origenes.rs` 137, `reinicio_historial.rs` 101. Ningún archivo supera 250.

## Archivos tocados

- `rust/crates/player/src/reproduccion.rs`
- `rust/crates/player/src/reproduccion/efectos.rs`
- `rust/crates/player/src/driver_reproduccion.rs`
- `rust/crates/player/tests/reinicio_historial.rs` (nuevo)
- `rust/crates/bot/src/api/eventos_reproduccion.rs`
- `rust/crates/bot/src/api/eventos_reproduccion_pruebas.rs` (nuevo)
- `rust/crates/bot/src/api/eventos_reproduccion_pruebas_origenes.rs` (nuevo)
- `rust/crates/bot/src/api/ciclo_vida.rs`
- `rust/crates/bot/src/api/proveedor_reproduccion.rs`

No se hizo commit, stash, reset ni checkout. El código de Rust no está en Git, así que el cambio se compara con el respaldo en `C:/Users/Usuario/Documents/Proyectos/ignite/backup-ignite-music-2026-11-19`.
