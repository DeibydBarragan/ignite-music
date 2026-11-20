# M7: salir del canal de voz si falla el arranque al restaurar

## Qué hace Node al fallar

En `src/music/sessions-restore.js`, `restoreSessions` llama a `manager.withPlayLock` y dentro hace `distube.queues.create(voiceChannel, ...)`, luego `queue.play()`. Si `play` lanza error, el `catch` solo registra un aviso. El `finally` borra la sesión solo si `manager.getQueue(guildId)` es falso.

En `node_modules/distube/dist/index.js` (`QueueManager.create`, líneas 2013-2022), `create` une el bot al canal con `await voice.join()` y solo después añade la cola. Por eso:

- Si `create` falla al unirse: no hay cola ni conexión. Node borra la sesión y no queda unido.
- Si el fallo llega después de `create` (por ejemplo en `queue.play()`): la cola ya existe. El `finally` no borra la sesión y Node no llama a `leave`. El bot queda unido. No verifiqué si DisTube detiene la cola al manejar el error de reproducción (`#handlePlayingError`, sin leer); si lo hace, la cola deja de existir y el resultado depende de ese paso.

Esto corrige lo que dice `docs/evidencia/F1-restaurar-sin-borrar.md`: Node borra la sesión solo cuando no hay cola, no en cualquier fallo.

## Qué hacía Rust

En `rust/crates/bot/src/restauracion.rs`, `arrancar_reproduccion` une el driver con `drv.join` y luego llama a `iniciar_pista_desde_con_settings`. Si esa llamada falla, el `?` devuelve el error sin salir del canal. `restaurar_sesiones` conserva la sesión (F1), pero el bot quedaba unido sin reproducir.

## Cambio

- `rust/crates/player/src/sesiones_restaurar.rs`: el trait `ProveedorRestauracion` tiene un nuevo método `salir_de_voz`, con implementación por defecto vacía. `restaurar_sesiones` lo llama cuando el estado es `ArranqueFallido`. Se llama después de que `restaurar_servidor` suelta el bloqueo de la cola, así no se toma ningún bloqueo de cola durante la salida.
- `rust/crates/bot/src/restauracion.rs`: `ProveedorRestauracionReal::salir_de_voz` llama a `drv.leave(gid)`, la misma primitiva que usan `/stop` y el apagado (`acciones_voz.rs`, `apagado.rs`). Si falla, registra un aviso.

No se tocó la sesión guardada. Sigue conservándose como en F1.

## Pruebas

- `test_arranque_fallido_tras_unirse_sale_del_canal_y_conserva_sesion` en `rust/crates/player/tests/sesiones.rs`: un proveedor falso falla al arrancar y registra cada llamada a `salir_de_voz`. Comprueba que se sale del canal una vez para el servidor, que la sesión sigue cargable y que el archivo sigue en disco.
- Las 14 pruebas previas de `sesiones` siguen pasando.

Límite: la prueba usa un proveedor falso. No cubre `ProveedorRestauracionReal::salir_de_voz` ni `drv.leave` contra Songbird, porque el driver real necesita Discord. No se probó la salida real del canal de voz.

## Comandos y resultados

Desde `rust/`, con `cargo` de `%USERPROFILE%\.cargo\bin`:

- `cargo test -p player --locked --test sesiones`: 15 pruebas pasan, 0 fallan.
- `cargo fmt --check`: exit 0.
- `cargo clippy --all-targets --locked -- -D warnings`: exit 0.
- `cargo test --workspace --locked`: exit 0, ninguna línea `FAILED` ni `panicked`.
- `cargo build --release --locked`: exit 0.

Desde la raíz, `npm test`: 235 pruebas, 228 pasan, 0 fallan, 7 omitidas.

Longitudes: `bot/src/restauracion.rs` 139 líneas, `player/src/sesiones_restaurar.rs` 160 líneas. Ambos son menores de 250. `player/tests/sesiones.rs` tiene 534 líneas; es un archivo de pruebas y ya tenía ese tamaño antes del cambio.

## Riesgos y no verificado

- Salida real de Discord: no probada.
- Si DisTube detiene la cola tras un error de reproducción en Node, no lo confirmé. Solo leí la ruta de `create`.
- El texto del aviso de log no se prueba; las pruebas no tienen suscriptor de `tracing`.
- Sin commit, stash ni reset.
