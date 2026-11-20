# Revisión R3: plan de pendientes menores

Rama `feature/rust-migration`, HEAD `9d5686d`. Solo lectura. El árbol tiene cambios sin commit, que no se tocaron. Ningún archivo se modificó salvo este informe.

## Resumen

| Ítem | Premisa | Veredicto | Decisión que falta |
|---|---|---|---|
| M1 | Patrón de ID doble escapado | Falsa. Ya está bien | Cerrar sin cambios |
| M2 | Validar rust.yml con parser instalado | No hay parser instalado | Instalar uno, o aceptar revisión manual |
| M3 | reset sin clave de servicio | Cierta. Diverge de las otras escrituras | Si el reinicio debe fallar (503) |
| M4 | `datos/anuncio.rs` es código muerto | Falsa en la ruta. Anunciador está vivo | Borrar solo `ClienteSupabaseHttp` |
| M5 | Rust 80 vs Node 100 | Cierta en el valor. Cambia comportamiento | Confirmar el cambio de volumen por defecto |
| M6 | Prueba de POST /play por el handler | Cierta. Fuera de alcance según F3 | Aceptar sin prueba o ampliar seam |
| M7 | Bot queda unido a voz tras fallo | Cierta en el código. Sin prueba en runtime | Confirmar la corrección |

## Detalle

**M1, falsa.** Cada patrón de `openapi.yaml` tiene una sola barra invertida. Lo comprobé línea a línea: 72, 73, 395, 562, 563, 632, 638, 683, 689, 702 y 792. `HEAD` también. `node --test test/openapi.test.js` pasa 2 de 2 (`test/openapi.test.js:72-82`). `docs/evidencia/T7-openapi.md` dice lo contrario. Ese dato está desactualizado.

**M2, parser ausente.** No hay PyYAML en Python 3.13. No hay paquete `yaml` ni `js-yaml` en `node_modules`. Tampoco hay caché de `npx`. `.github/workflows/rust.yml` tiene 73 líneas. El job `msrv` existe. Sin parser, la validación es manual. Instalar un parser es una acción que necesita aprobación.

**M3, cierta.** `rust/crates/bot/src/datos/ajustes_voz.rs:224-236`: el reinicio ignora errores de borrado (`if let Ok(req)` y `let _ = req.send()`). Devuelve los valores por defecto aunque no borre nada. Las otras escrituras sí fallan: `ajustes_voz.rs:171-177` devuelve `503 DB_UNAVAILABLE`. `rust/crates/bot/src/api/rutas/voz.rs:63-72` responde siempre 200. Node también traga el error (`src/voice/settings-store.js:239-249`). `docs/evidencia/T4-supabase.md:66` pide decidir si el reinicio debe fallar. Corregirlo cambia el contrato HTTP y deja de coincidir con Node.

**M4, premisa falsa.** `rust/crates/bot/src/datos/anuncio.rs` no existe. El módulo es `rust/crates/bot/src/api/anuncio.rs`. `Anunciador` sí está vivo: `api/ciclo_vida.rs:153` lo construye. Solo `ClienteSupabaseHttp` (`api/anuncio.rs:33-50`) no tiene uso fuera de su módulo. Lo reexporta `api/mod.rs:28`. Borrar esa reexportación cambia la API pública del crate. Borrar `Anunciador` rompería `tests/anuncio.rs` y el apagado.

**M5, cierta, con efecto visible.** Node usa `config.bot.defaultVolume`, que vale 100 (`src/config.js:77`). Lo aplica en `src/music/events.js:80` y `sessions-restore.js:41`. Rust fija 80 en `rust/crates/player/src/cola.rs:99`. Ese valor también sale en `reproductor.rs:50` (cola vacía) y en `sesiones.rs:11`. La variable `DEFAULT_VOLUME` se lee en `nucleo/src/config.rs:106`, pero la cola no la usa. Cambiar a 100 altera el volumen de cada cola nueva. Hay pruebas que esperan 80: `bot/tests/rutas_v1.rs:105`, `bot/tests/panel_lifecycle.rs:89`, `bot/tests/panel_paridad.rs:35` y `player/tests/driver_unit.rs:221`. La restauración usa el volumen guardado (`sesiones_restaurar.rs:119`), así que no cambia. Según `pref.preserve_existing_behavior`, conviene confirmar antes.

**M6, cierta.** `docs/evidencia/F3-limpieza.md:30-42`: el test de POST /play necesita sembrar caché y simular la unión a voz. F3 estima que supera las ~40 líneas autorizadas. Es una decisión del conductor.

**M7, cierta en el código.** `rust/crates/bot/src/restauracion.rs:109-114` une el driver al canal antes de iniciar la pista. Si `iniciar_pista_desde_con_settings` falla (líneas 115-124), el `?` devuelve error sin salir del canal. No hay llamada a salir en `restauracion.rs`. `rust/crates/player/src/sesiones_restaurar.rs:149-158` limpia la cola, pero no sale del canal. Node en `sessions-restore.js:53-56` solo registra el error. No verifiqué en runtime cómo se comporta DisTube.

## Solapes y orden

M5 toca `cola.rs` y `reproductor.rs`. M3 toca `ajustes_voz.rs`. M7 toca `restauracion.rs` y `sesiones_restaurar.rs`. M6 toca el proveedor de reproducción. No hay archivo compartido entre M3, M5 y M7. M7 y M6 comparten la ruta de restauración y de reproducción, así que conviene que sean en serie. M4 y M1 no tocan Rust.

## Riesgos

- M5 cambia el volumen por defecto de cada cola nueva. El usuario lo percibe.
- M3 cambia el código HTTP de reinicio. Node no lo hace.
- M7 deja de estar unido a voz tras un fallo. No hay prueba de runtime.
- M4 borrado parcial puede romper la API pública del crate.

## Qué falta

- Decisión sobre M2 (instalar parser o revisar a mano).
- Decisión sobre M3 (si el reinicio debe fallar).
- Confirmación de M5 (volumen por defecto a 100).
- No ejecuté `cargo test` ni la validación de rust.yml. Solo leí código.
