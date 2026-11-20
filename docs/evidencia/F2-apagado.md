# F2: guardar sesiones en el apagado sin depender de túnel ni anunciador

Fecha: 2026-11-19. Rama `feature/rust-migration`, base `9d5686d`. No hay commit, stash, reset ni checkout. No se instaló nada.

## Problema

En `rust/crates/bot/src/arranque.rs`, todo el apagado iba dentro de un `timeout` de 10 s. Dentro iban `servicios.apagar()` (posición, anunciador, túnel y API con hasta 2 s) y después `ejecutar_apagado`, que guarda las sesiones. Si el túnel o el anunciador tardaban, el timeout externo cancelaba el guardado. Así se cumplía el riesgo 2 de `docs/evidencia/VERIFICACION-R1.md`.

## Orden de apagado en Node

El código está en `src/setup/launcher.js` y `src/bot.js`.

1. `launcher.js`: para el túnel (`tunnel.stopTunnel()`), solo si `autoTunnel` está activo.
2. `launcher.js`: envía `shutdown` a cada shard y espera como máximo `SHUTDOWN_GRACE_MS` = 10 s.
3. `bot.js` `client.shutdown`: para el anunciador (`clearInterval`).
4. `bot.js`: cierra el servidor API sin esperarlo (`apiServer.close()` y `closeAllConnections()`).
5. `bot.js`: `await client.music.shutdown()`, que guarda las sesiones.
6. `bot.js`: vuelca los CSV de voz.

En Node, el guardado de sesiones no espera al cierre de la API. El plazo de 10 s es un plazo total del proceso, no un límite que el guardado comparta con los servicios.

## Orden en Rust antes del cambio

1. Cierra el hub SSE.
2. Dentro de un timeout de 10 s, en serie: `servicios.apagar()` (posición, anunciador, túnel, API con aborto de 2 s), y después `ejecutar_apagado` (despacho, sesiones, CSV, gateway).

## Orden en Rust después del cambio

1. Cierra el hub SSE (sin cambio).
2. Dentro del timeout de 10 s, en paralelo con `tokio::join!`:
   - `ejecutar_apagado`: despacho, sesiones (con salida de canales de voz), CSV y gateway. Empieza al instante, porque los dos primeros pasos no hacen esperas.
   - `servicios.apagar()`, con su propio plazo de 6 s (`PLAZO_SERVICIOS_MS`). Si se cumple, el futuro se corta y se continúa.
3. El código de salida sigue siendo el mismo: 0 si el apagado termina, 1 si el timeout externo o un fallo del gateway lo interrumpe.

Lo que cambia para el dashboard: el hub SSE sigue cerrándose antes de todo, igual que antes. El gateway ahora se cierra mientras la API todavía se apaga, en vez de después. Eso se documenta como `shortcut:` en el código.

## Archivos

- `rust/crates/bot/src/arranque.rs`: nueva función pública `apagar_con_servicios` (genérica sobre `PasosApagado`, para poder probarla con dobles), constante `PLAZO_SERVICIOS_MS` y uso desde `supervisar_gateway`. 143 líneas.
- `rust/crates/bot/tests/apagado_orden.rs` (nuevo): dos pruebas con doble de `PasosApagado`.
- Sin cambios: `apagado.rs` (95 líneas), `api/ciclo_vida.rs` (198 líneas), y el resto del código de Rust y de Node.

## Pruebas

| Prueba | Archivo | Resultado |
|---|---|---|
| (a) servicio colgado (túnel que nunca termina): las sesiones se guardan y el apagado termina en menos de 1 s | `tests/apagado_orden.rs::tunel_colgado_no_impide_guardar_sesiones` | pasa |
| (b) apagado normal: guarda, cierra servicios y pasos en orden despacho, sesiones, csv, gateway | `tests/apagado_orden.rs::apagado_normal_guarda_y_cierra_todo` | pasa |
| (c) aborto de 2 s del servidor API (T8): cliente SSE parado, termina antes de 10 s | `tests/sse_payload.rs` (apagado con cliente SSE parado) | pasa, 4 de 4 en el archivo |

## Comandos y conteos

Todos se ejecutaron en `rust/` o en la raíz de `ignite-music`.

- `cargo fmt --check`: sale con 0 tras formatear solo `arranque.rs` y el archivo nuevo con `rustfmt`.
- `cargo clippy --all-targets --locked -- -D warnings`: sale con 0.
- `cargo test --workspace --locked`: 84 resultados de prueba, 348 pasan, 0 fallan, 2 ignoradas.
- `cargo build --release --locked`: sale con 0.
- `npm test` en la raíz: 234 pruebas, 227 pasan, 0 fallan, 7 omitidas. Node no cambió.

## No verificado

- Un apagado real con bot, Discord, Supabase y túnel cloudflared. Las pruebas usan dobles.
- El dashboard con un navegador.
- Si el corte de 6 s deja el túnel o la API a medio cerrar en un caso real. El proceso termina de todos modos, pero no se midió.

## Riesgos

- Si el anunciador o el túnel superan 6 s, su parada queda incompleta. El guardado de sesiones no se ve afectado, pero el túnel podría seguir vivo un instante.
- El gateway se cierra en paralelo con la API. Un `POST /play` que llegue en ese momento puede fallar en vez de quedar en cola.
- Sin revisar: el ticker de posición (`sincronizar_posicion`) y el riesgo de borrar la sesión al fallar el arranque (F1, otro ítem).
