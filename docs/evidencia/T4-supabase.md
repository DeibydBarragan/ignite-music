# T4: credenciales de escritura de Supabase

**Estado:** implementado en el árbol de trabajo. Sin commit, stash, reset ni checkout. Rama `feature/rust-migration`, HEAD `9d5686d`.
**Respaldo:** `C:/Users/Usuario/Documents/Proyectos/ignite/backup-ignite-music-2026-11-19`.

## Decisión

Las escrituras exigen `SUPABASE_SERVICE_ROLE_KEY`. Sin ella devuelven `SinCredencialEscritura` y no abren ninguna conexión HTTP. Lo piden S-30 y el hallazgo 10 de la auditoría.

**Divergencia deliberada de Node.** Node usa `SUPABASE_KEY` o, si falta, `SUPABASE_ANON_KEY` (`src/config.js:101`) y escribe con esa clave (`src/core/supabase.js`). La RLS de `20261016_lockdown_rls.sql` ya rechaza esas escrituras en el servidor.

Las lecturas no cambian. Con clave de servicio, siguen usando la clave de servicio, como antes. Sin ella, usan `SUPABASE_KEY` o `SUPABASE_ANON_KEY`.

## Cambios

| Archivo | Cambio |
| --- | --- |
| `rust/crates/bot/src/datos/supabase.rs` | Nuevo `SinCredencialEscritura`. `con_claves(url, lectura, escritura)`. `post`, `patch` y `delete` devuelven `Result`. `rpc` (`top_songs`) usa la clave de lectura. Aviso de arranque si falta la clave de servicio. |
| `rust/crates/bot/src/datos/ajustes_voz.rs` | Error de escritura de ajustes → 503 `DB_UNAVAILABLE`, caché intacta (S-30). Borrado de bindings y reinicio ignoran el fallo, como antes. |
| `rust/crates/bot/src/datos/favoritos.rs` | Error de escritura → 500 `FAVORITE_INSERT_FAILED` o `FAVORITE_DELETE_FAILED`, igual que un fallo de red. |
| `rust/crates/bot/src/datos/historial.rs` | Error de escritura → `debug!`, la reproducción continúa. |
| `rust/crates/bot/src/datos/listas.rs` | Error de escritura → 500 `PLAYLIST_INSERT_FAILED` o `PLAYLIST_UPDATE_FAILED`. |
| `rust/crates/bot/tests/supabase_escritura.rs` | Nuevo. Cuatro pruebas (abajo). |

`anuncio.rs` no se tocó. Su `ClienteSupabaseHttp` tiene su propia escritura con la clave que recibe, pero nadie lo construye fuera de su impl. Parece código muerto. Conviene borrarlo en una tarea aparte.

`ClienteSupabaseRest::nuevo(url, key)` sigue usando una sola clave para lectura y escritura. Lo usan los tests existentes de `tests/datos.rs`. Producción solo llama a `desde_config`, que usa `con_claves`.

Tamaño de archivos de producción tras los cambios: `supabase.rs` 203, `ajustes_voz.rs` 242, `favoritos.rs` 136, `historial.rs` 213, `listas.rs` 152. Todos bajo 250.

Rutas HTTP: ningún contrato cambió. Un fallo de escritura sin clave llega a las rutas con el mismo código de error que un fallo de Supabase.

## Aviso de arranque

Si `SUPABASE_URL` y `SUPABASE_KEY` están definidos pero falta `SUPABASE_SERVICE_ROLE_KEY`, el log muestra `SUPABASE_SERVICE_ROLE_KEY is not set: Supabase writes are disabled, reads still work`. No muestra valores de claves. `desde_config` se llama dos veces al arrancar (`ciclo_vida.rs` y `contexto.rs`), así que el aviso aparece dos veces.

## Pruebas (`rust/crates/bot/tests/supabase_escritura.rs`)

| Prueba | Qué comprueba |
| --- | --- |
| (a) `escritura_sin_clave_de_servicio_no_abre_conexion` | Escritura sin clave de servicio: error tipado y ninguna conexión al servidor local. |
| (b) `escritura_con_clave_de_servicio_usa_esa_clave` | Con clave de servicio, la escritura llega con `Bearer` de servicio y sin la clave de lectura. |
| (c) `lectura_funciona_con_clave_de_lectura_sin_servicio` | Sin clave de servicio, `verify_connection` lee con la clave de lectura. |
| (d) `el_error_de_escritura_no_contiene_claves` | El mensaje de error no contiene ninguna clave. |

Las pruebas usan un servidor HTTP local de una conexión. No consultan Supabase real.

## Gates

| Gate | Resultado |
| --- | --- |
| `cargo fmt --all -- --check` | Pasa. Se aplicó `cargo fmt --all` solo a los archivos de esta tarea. |
| `cargo clippy --all-targets --locked -- -D warnings` | Pasa, sin avisos. |
| `cargo test --workspace --locked` | 339 aprobadas, 0 fallan, 2 ignoradas (`recepcion_manual` y `driver_manual`). Conteo medido ahora, no comparado con una corrida previa. |
| `cargo test -p bot --test supabase_escritura --test datos` | 4 nuevas y 6 de datos, todas aprobadas. |
| `cargo build --release --locked` | Pasa. `rust/target/release/bot.exe` SHA-256 `A688D5226F633B178C997181FB68E8DE6CC9D3402C7EEB839ECA511DFBD5803A`. |
| `npm test` (raíz) | 234 pruebas, 227 aprobadas, 0 fallan, 7 omitidas. Igual que antes del cambio. |

## Configuración del `.env`

`ignite-music/.env` **sí** define `SUPABASE_SERVICE_ROLE_KEY`. Se comprobó solo el nombre. El valor no se leyó ni se registró. Con esa clave, el corte no rompe las escrituras de Rust. Solo una escritura real puede confirmar que la clave es válida.

## Lo que no se verificó y riesgos

- No se probó contra Supabase real. Las escrituras con la clave de servicio siguen sin confirmar en el servidor.
- `reset_guild_settings` sin clave de servicio no borra filas en Supabase. Limpia la caché y devuelve los valores por defecto. Al expirar la caché, los valores guardados vuelven. Es el mismo trato que ya daba a un fallo de borrado. Conviene decidir si el reinicio debe fallar.
- Los callers de bindings en `update_guild_settings` solo corren si la escritura de ajustes funciona. Sin clave de servicio, esa escritura ya falla antes.
- Node no cambió. La divergencia queda documentada aquí.
