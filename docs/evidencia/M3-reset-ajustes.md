# M3: reinicio de ajustes de voz sin clave de servicio

**Estado:** implementado en el árbol de trabajo. Sin commit, stash, reset ni checkout. Rama `feature/rust-migration`, HEAD `9d5686d`.

## Decisión

El reinicio sigue la misma regla que las demás escrituras (T4). Sin `SUPABASE_SERVICE_ROLE_KEY` no hace ninguna petición HTTP. El contrato HTTP no cambia: `POST`/`DELETE` de reinicio responden 200 con los valores por defecto, como Node.

## Cambio

| Archivo | Cambio |
| --- | --- |
| `rust/crates/bot/src/datos/ajustes_voz.rs` | `reset_guild_settings` ya no ignora errores. Cada `DELETE` se comprueba. Sin clave, o si el borrado falla, se escribe un `warn!` con el motivo. Sin valores ni claves en el log. Se añade `use tracing::warn`. |
| `rust/crates/bot/tests/ajustes_voz_reset.rs` | Nuevo. Cuatro pruebas (abajo). |

Un reinicio sin clave deja dos avisos en el log, uno por cada `DELETE`.

## Pruebas (`rust/crates/bot/tests/ajustes_voz_reset.rs`)

| Prueba | Qué comprueba | Resultado |
| --- | --- | --- |
| `reinicio_sin_clave_de_servicio_no_abre_conexion` | Sin clave: cero conexiones y valores por defecto. | Pasa |
| `reinicio_con_clave_de_servicio_borra_con_esa_clave` | Con clave: dos `DELETE` con `Bearer` de servicio y sin la clave de lectura. | Pasa |
| `fallo_de_borrado_http_devuelve_valores_por_defecto` | Respuesta 500 en ambos borrados: valores por defecto, sin pánico. | Pasa |
| `fallo_de_conexion_al_borrar_no_rompe_el_reinicio` | Puerto cerrado: valores por defecto, sin pánico. | Pasa |

La prueba de la respuesta HTTP 200 se hace a nivel de gestor. El handler de `rust/crates/bot/src/api/rutas/voz.rs` no cambió y sigue respondiendo 200.

## Gates

| Gate | Resultado |
| --- | --- |
| `cargo fmt --all -- --check` | Pasa. Tras formatear el archivo de pruebas nuevo. |
| `cargo clippy --all-targets --locked -- -D warnings` | Pasa, sin avisos. |
| `cargo test --workspace --locked` | Todos los binarios pasan, 0 fallan. Las ignoradas son las mismas de siempre (`recepcion_manual`, `driver_manual`). |
| `cargo test -p bot --test ajustes_voz_reset` | 4 aprobadas. |
| `cargo build --release --locked` | Pasa. |
| `npm test` (raíz) | 235 pruebas, 228 aprobadas, 0 fallan, 7 omitidas. El recuento cambió respecto a T4 (234/227). Node no se tocó en esta tarea. |

Tamaño de archivos de producción: `ajustes_voz.rs` 248 líneas, bajo el límite de 250.

## No verificado

- Supabase real. Las pruebas usan un servidor HTTP local.
- El texto exacto de los avisos en un archivo de log en disco. Solo se comprobó que compila.
- Que el handler HTTP devuelva 200 en la ruta de reinicio, más allá de lo que muestra el código.
