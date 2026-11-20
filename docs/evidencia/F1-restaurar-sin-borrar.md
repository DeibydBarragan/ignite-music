# F1: no borrar la sesión guardada si falla el arranque al restaurar

## Qué hace Node

En `src/music/sessions-restore.js`, el `catch` solo registra un aviso. El bloque `finally` borra la sesión guardada con `sessions.remove(guildId)` cuando no existe cola activa. Por eso Node borra la sesión también cuando falla el arranque de la pista. Esta divergencia es deliberada: Rust conserva la sesión.

La caducidad es la misma en ambos lados: 30 minutos desde `savedAt` (`src/music/session.js`, `MAX_AGE_MS`; `rust/crates/player/src/sesiones.rs`, `MAX_AGE_MS`).

## Qué cambió en Rust

En `rust/crates/player/src/sesiones_restaurar.rs`, `restaurar_servidor` devuelve un estado (`Restaurada`, `Descartada`, `ArranqueFallido`) en lugar de `bool`. Si `arrancar_reproduccion` falla, el estado es `ArranqueFallido` y `restaurar_sesiones` no borra el archivo. El aviso del log indica que la sesión se conserva para reintentar en el siguiente arranque. El error se registra como antes; no se añaden claves al log.

Las salidas por sesión inválida, sin oyentes, sin canciones o caducada siguen devolviendo `Descartada`, y el archivo se borra como antes.

## Comportamiento de borrado al restaurar con éxito

El código previo no borraba el archivo cuando la restauración funcionaba, porque la cola queda activa. Ese comportamiento se mantiene. El archivo se sobrescribe en el siguiente guardado. El texto del encargo decía que se borraba en éxito; no se cambió porque el encargo pedía preservar el comportamiento actual.

## Archivos tocados

- `rust/crates/player/src/sesiones_restaurar.rs`: 154 líneas (límite 250).
- `rust/crates/player/tests/sesiones.rs`: pruebas nuevas (3) y un proveedor que falla.
- `rust/crates/player/src/sesiones.rs`: sin cambios, 246 líneas.
- `docs/evidencia/F1-restaurar-sin-borrar.md`: este documento.

No se tocó Node. No hubo commit, stash, reset ni checkout.

## Pruebas

- (a) Arranque fallido: el archivo sigue en disco y `cargar_con_tiempo` lo devuelve. Prueba `test_arranque_fallido_conserva_sesion_y_reintenta_al_reiniciar`.
- (b) Arranque exitoso: la prueba comprueba que el archivo sigue existiendo, como antes. Prueba `test_arranque_exitoso_no_borra_sesion_como_antes`.
- (c) Reinicio con arranque funcionando: restaura la sesión conservada con la pista en la posición 40. Prueba `test_arranque_fallido_conserva_sesion_y_reintenta_al_reiniciar`.
- Caducidad: una sesión conservada con más de 30 minutos se descarta y se borra. Prueba `test_sesion_conservada_caducada_se_descarta`.

## Comandos y resultados

Comandos ejecutados en `rust/` con `cargo` de `%USERPROFILE%\.cargo\bin`:

- `cargo fmt --check`: exit 0.
- `cargo clippy --all-targets --locked -- -D warnings`: exit 0.
- `cargo test --workspace --locked`: exit 0, 346 pruebas pasan, 0 fallan, 1 ignorada.
- `cargo build --release --locked`: exit 0.
- `npm test` en la raíz: 234 pruebas, 227 pasan, 0 fallan, 7 omitidas.

## Riesgos

- La prueba de log no verifica el texto del aviso; no hay suscriptor de tracing en las pruebas. Queda sin probar.
- Una sesión conservada puede quedar en disco hasta 30 minutos. Después la caducidad la descarta, o la sobrescribe un guardado nuevo.
- Node sigue borrando en este caso. Si el servidor falla de nuevo al reiniciar, Rust volverá a conservar la sesión.
