# T6: /play y /playnext responden con defer antes de la unión de voz

## Corrección de la premisa del revisor

El revisor dijo que no aparece `defer` en `rust/crates/bot/src/comandos/reproducir.rs`. Eso es cierto, pero el defer está antes, en el despacho:

- `rust/crates/bot/src/interacciones_comandos.rs:25-30`: para `play`, `playnext` y `stop` se llama a `diferir(...)` (DeferredChannelMessageWithSource, efímero). Después se toma `tomar_turno_voz`.

Camino completo de `/play` y `/playnext`:

1. `rust/crates/bot/src/cliente.rs`, `Event::InteractionCreate`: cada interacción va en su propia tarea del `JoinSet` (`cliente_trabajos.rs`). El bucle del gateway no espera a la interacción.
2. `rust/crates/bot/src/interacciones.rs:168` (`despachar`) llama a `ejecutar_comando_completo`.
3. `interacciones_comandos.rs:25-30`: `diferir` (defer) y luego `tomar_turno_voz`.
4. `interacciones_comandos.rs:57-59`: guardia de voz (`validar_guard_voz`), sin esperar la unión.
5. `interacciones_comandos.rs:68-69`: llama a `ejecutar_cmd_play`.
6. `interacciones_comandos_aux.rs:157`: `drv.join(...)`. Si falla, avisa con `musicplayer.failed_connect_voice` y hace `return`. No encola.
7. `interacciones_comandos_aux.rs:171`: `reproducir_desde_interaccion` (`comandos/reproducir.rs:84`), que encola la pista.

Los eventos de voz (`VoiceStateUpdate`, `VoiceServerUpdate`) los procesa el bucle principal de `cliente.rs` con `process_gateway_event`. No espera al manejador de la interacción, así que llegan durante la unión. `driver_conexion.rs:55` (`join`) espera `VoiceServerUpdate` con timeout de 20 s.

Única espera compartida: `tomar_turno_voz` (`interacciones.rs:59`) es un mutex por servidor. Un segundo `/play` o `/stop` del mismo servidor recibe el defer al instante, pero su respuesta final sale cuando termina el primero. Otros servidores no se bloquean.

## Cambios

- Producción: ninguno. El camino real no cambia.
- Pruebas nuevas: `rust/crates/bot/tests/defer_play.rs` (2 pruebas).
  - `play_responde_con_defer_antes_de_que_termine_la_union`: (a). El shard sigue vivo, así que la unión espera `VoiceServerUpdate`. El test recibe el POST de defer (`type` 5) mientras el manejador sigue pendiente, y comprueba que no sale ningún otro envío.
  - `union_fallida_avisa_al_usuario_y_no_encola_la_pista`: (c). El shard se cierra, así que la unión falla al instante. El test comprueba: defer, luego `PATCH .../messages/@original` con el aviso de error, y la cola sin canciones.
- Seam: no hace falta. El cliente HTTP de twilight usa `proxy(url, true)` hacia un servidor local que graba las llamadas (`tests/defer_play.rs`, `servidor_mock`). Twilight tiene ese método en la versión 0.17.1 del lock.
- Pruebas existentes: `rust/crates/bot/src/cliente/pruebas.rs`, `el_gateway_entrega_ambos_eventos_mientras_el_comando_espera`. Cubre (b) y (d) con un doble de router, no el manejador real.

## Gates (desde `rust/`)

- `cargo fmt --check`: exit 0. Antes había diferencias solo en el archivo nuevo; se formateó ese archivo.
- `cargo clippy --all-targets --locked -- -D warnings`: exit 0.
- `cargo test --workspace --locked`: antes 319 pasan, 2 ignoradas. Después 321 pasan, 2 ignoradas, 0 fallos.
- `cargo build --release --locked`: exit 0.
- `scripts/check-source-size.ps1`: OK, todos los módulos Rust de `src` tienen 250 líneas o menos.

## Sin hacer

- No se hizo prueba de mutación (quitar el defer y ver que falla). El test (a) espera el POST de defer antes de que termine la unión, así que sin defer habría fallado en la espera.
- No se probó `/playnext` por separado: usa el mismo camino `ejecutar_comando_completo`.
- No se hizo commit, `add`, `stash`, `reset`, `clean`, `checkout` ni `push`.
