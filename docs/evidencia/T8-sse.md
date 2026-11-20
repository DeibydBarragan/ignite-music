# T8: payload SSE igual a GET /player y apagado con cliente SSE parado

Fecha: 2026-11-19. Rama `feature/rust-migration`, base `9d5686d`. No hay commit, stash, reset ni checkout.

Respaldo previo a editar: `C:/Users/Usuario/Documents/Proyectos/ignite/backup-ignite-music-2026-11-19-T8`. La carpeta `backup-ignite-music-2026-11-19` ya existía y tiene un estado anterior; no se tocó.

## Resultado de las pruebas pedidas

| Prueba | Archivo | Resultado antes del cambio | Resultado después |
|---|---|---|---|
| (a) evento inicial == cuerpo de GET /player | `rust/crates/bot/tests/sse_payload.rs` | pasa | pasa |
| (b) player.sync tras un cambio == GET /player | `rust/crates/bot/tests/sse_payload.rs` | pasa | pasa |
| (c) sincronización nunca emite cuerpo vacío | `rust/crates/bot/tests/sse_payload.rs` | pasa | pasa |
| (d) apagado con cliente SSE parado termina antes de 10 s | `rust/crates/bot/tests/sse_payload.rs` | falla: bloqueado más de 20 s | pasa |

## Hallazgos

Los puntos (a) y (b) ya cumplían. Rust construye el evento inicial, cada `player.sync` y `GET /player` con la misma función, `calcular_estado_reproductor` (`rust/crates/bot/src/api/rutas/reproductor.rs`). No hay un formato distinto entre SSE y GET dentro de Rust.

El punto (c) también cumplía. El ticker de 2 s (`sincronizar_posicion` en `eventos_reproduccion.rs`, que no es un archivo de este ítem) llama a `publicar_sincronizacion`. Esa función siempre envía un objeto JSON. La prueba cubre esa ruta, no el bucle del ticker.

El punto (d) no cumplía. `arranque.rs` cierra el hub SSE y después espera `ServiciosApiBot::apagar`, que esperaba a Axum sin plazo. Un cliente que no lee deja el socket lleno, y Axum no termina. Con el límite de 10 s, el apagado se corta antes de guardar sesiones.

## Cambios

- `rust/crates/bot/src/api/ciclo_vida.rs`: `apagar` cierra el hub antes de parar el servidor. Nueva función pública `apagar_servidor_acotado`: envía la señal de cierre, espera 2 s (`PLAZO_CIERRE_SERVIDOR_API`) y, si Axum no termina, aborta la tarea. Lleva el comentario `shortcut:`.
- `rust/crates/bot/tests/sse_payload.rs` (nuevo): cuatro pruebas de (a) a (d). La prueba (d) reproduce el bloqueo: 80 eventos de 256 KiB sin lectura del cliente.
- `rust/crates/bot/src/api/sse.rs`, `rust/crates/bot/src/panel/instantanea.rs`: sin cambios. No hizo falta cambiar el payload.

Comprobación: después de `cargo fmt -p bot`, la comparación con el respaldo T8 muestra solo estos dos archivos cambiados.

## Comandos y conteos

Todos se ejecutaron en `rust/` o en la raíz del repositorio:

- `cargo fmt --check`: sale con 0 tras aplicar `cargo fmt -p bot`.
- `cargo clippy --all-targets --locked -- -D warnings`: sale con 0.
- `cargo test --workspace --locked`: 83 suites, 343 pruebas pasan, 0 fallan, 2 ignoradas.
- `cargo test -p bot --locked --test sse_payload`: 4 de 4 pasan.
- `cargo build --release --locked`: sale con 0.
- `npm test` en la raíz: 234 pruebas, 227 pasan, 0 fallan, 7 omitidas. Node no cambió.

## No verificado

- Dashboard real: no se abrió el panel web con un navegador.
- Discord y Supabase reales: no se conectó ningún bot.
- Guardado de sesiones de extremo a extremo: la prueba (d) cubre el cierre del servidor. Que `PasosApagadoReal` guarde las sesiones dentro de los 10 s no se ejecutó con un bot real.
- Ticker de 2 s: no se probó el bucle de `eventos_reproduccion.rs`, que no es un archivo de este ítem.

## Riesgos y diferencias con Node (sin cambiar)

- Volumen sin cola: Rust usa 80 fijo en `estado_vacio`. Node usa `config.bot.defaultVolume`, cuyo valor por defecto es 100 (`DEFAULT_VOLUME`). Esto afecta a `GET /player` y al SSE por igual. Corregirlo toca `formas`/`reproductor.rs`, fuera de los archivos de este ítem.
- Sin cola: Rust incluye `"voiceChannel": null`. Node omite esa clave. Mismo caso: fuera del alcance.
- Carrera en la conexión: `get_events` (en `reproductor.rs`) calcula el estado inicial antes de suscribirse. Un cambio entre ambos pasos no llega al cliente hasta el siguiente cambio. Corregirlo exige editar `reproductor.rs`.
- Sin verificar, como se indicó arriba: dashboard, Discord, Supabase, sesiones de extremo a extremo y el bucle del ticker.
