# M6: historial de POST /play por el handler real

## Resultado

La parte B de F3 queda cubierta. Dos pruebas llaman al handler real `post_play` (`rust/crates/bot/src/api/rutas/reproductor_play.rs`) y pasan:

- `post_play_registra_historial_una_vez`: una pista arranca por POST /play y el historial se registra una vez.
- `post_play_con_fallo_de_historial_sigue_reproduciendo`: el registrador falla, la respuesta sigue siendo 200 y la pista sigue activa en el driver.

## Puntos de inyección en producción

Dos campos nuevos en `ProveedorApiReal` (`rust/crates/bot/src/api/proveedor_real.rs`), en `None` y `false` por defecto:

- `prueba_resolucion`: sustituye la búsqueda o resolución con yt-dlp en `ejecutar_reproduccion_api`.
- `prueba_sin_union`: omite `conectar_canal_voz` (unión a voz de Discord).

Coste en producción: unas 24 líneas, dentro del límite de 120. Con los campos en su valor por defecto, el camino es el real. No cambian respuestas HTTP.

El resto del camino corre tal cual: validación de la petición, `seleccionar_canal` sobre una caché de Discord sembrada, encolado, `iniciar_pista_con_settings` sobre un `SongbirdDriver` real y el listener de `play`.

## Por qué hacían falta los puntos de inyección

`conectar_canal_voz` llama a `driver.join`, que espera hasta 20 s el handshake de voz de Discord y falla sin gateway. La resolución llama a yt-dlp y a la red.

## Hallazgo durante la prueba

El listener guarda solo un `Weak` del proveedor. El handler consume el `EstadoApiContexto`, así que el último `Arc` se libera al terminar la petición y el listener no registra. La prueba mantiene vivo el `Arc` del proveedor hasta el final. Producción no cambia; el listener sigue cumpliendo su función mientras la app vive el proveedor.

## Gates (19-nov-2026, rama feature/rust-migration)

| Gate | Resultado |
|---|---|
| `cargo fmt --check` | sin diferencias |
| `cargo clippy --workspace --all-targets -- -D warnings` | sin avisos |
| `cargo test --workspace` | todos pasan, 0 fallan, 1 ignorada en varios binarios |
| `cargo build --release --workspace` | termina en 2 min 57 s |
| `npm test` | 235 pruebas: 228 pasan, 0 fallan, 7 omitidas |

Tamaños: `proveedor_real.rs` 232 líneas, `proveedor_reproduccion.rs` 204 líneas, `post_play_historial_pruebas.rs` 218 líneas (archivo de pruebas).

## Lo que no se verificó

- Discord real: la unión de voz se omite con el punto de inyección. El handshake real no se probó.
- yt-dlp real: la resolución se sustituye. La búsqueda por URL o texto no se probó con el binario.
- Autoplay y Supabase: no se tocaron ni se probaron.
- Los archivos `rust/crates/bot` no están bajo git en este repo, así que no hay diff de git que revisar.
- No hubo commit, push, stash ni reset.
