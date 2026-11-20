# F3: límite de 250 líneas, POST /play y corte prematuro

Fecha: 2026-11-19. Rama `feature/rust-migration`. Sin commit, stash, reset ni checkout. Sin instalaciones. Node no se tocó.

## Resumen

| Parte | Estado |
|---|---|
| A. Límite de 250 líneas | Hecho |
| B. Prueba real de POST /play | No cubierta: un seam de resolución no basta (ver abajo) |
| C. Historial en corte prematuro | Hecho: Node también duplica, Rust no cambia, prueba de paridad |

## A. Límite de 250 líneas

Hallazgo: `rust/crates/voice/src/admission.rs` y `rust/crates/bot/src/api/rutas/voz.rs` tienen 252 líneas cada uno (conteo de saltos de línea). Una tabla previa de `Measure-Object` dio cifras menores y no sirve como referencia.

Lista de archivos de producción `rust/**/src/*.rs` de más de 250 líneas:

| Archivo | Antes | Después |
|---|---|---|
| `rust/crates/voice/src/admission.rs` | 252 | 147 |
| `rust/crates/bot/src/api/rutas/voz.rs` | 252 | 175 |

Cambios:
- `admission.rs`: las pruebas pasan a `rust/crates/voice/src/admission_pruebas.rs` con `#[path]`. Mismas tres pruebas, sin cambios de código.
- `voz.rs`: la validación del cuerpo de PUT /voice/settings pasa a `rust/crates/bot/src/api/rutas/voz_validacion.rs` como `validar_cuerpo_ajustes(&Value) -> Option<Response>`. Misma lógica y mismos mensajes de error, en el mismo orden.

Después de los cambios no hay ningún archivo de producción por encima de 250 líneas.

## B. POST /play real (pendiente)

La prueba `post_play_no_registra_dos_veces` no llamaba al handler. Se eliminó por engañosa: repetía `origen_comando_registra_una_vez`.

No se escribió la prueba del handler real. Estado final: B no cubierta.

Motivo. En `ejecutar_reproduccion_api` (`rust/crates/bot/src/api/proveedor_reproduccion.rs`), la resolución con yt-dlp va después de dos pasos que dependen de Discord:
1. `seleccionar_canal` (`proveedor_play_voz.rs`) lee la caché de guilds, canales y estados de voz.
2. `conectar_canal_voz` (`proveedor_real_operaciones.rs`) valida el canal y los permisos del bot en la caché, y después une el driver de voz.

Un seam solo en la resolución no basta para llegar al registro de historial por el router. Haría falta sembrar la caché y falsear la unión de voz. Eso supera los ~40 líneas de producción autorizados, así que no se hizo.

Lo que queda sin prueba: el registro de POST /play por el router. La cobertura del registro es la del listener (`origen_comando_registra_una_vez`, `origen_avance_...`, `origen_restauracion_...`). POST /play ya no registra directamente; esto se verificó leyendo el código (`proveedor_reproduccion.rs`), no con una prueba.

## C. Historial en corte prematuro

Node (`src/music/events.js`):
- `FINISH_SONG` (líneas 111-121): si el corte es prematuro y no hay reintento previo, fija `queue.repeatMode = 1` y `queue._beginTime = seekTarget`.
- DisTube reinicia la misma pista, así que emite `PLAY_SONG`.
- `PLAY_SONG` (línea 102): llama a `history.recordPlay` de nuevo.

Conclusión: Node registra dos veces. Rust hace lo mismo: `comprobar_corte_prematuro` cambia el modo a `Cancion` y el reinicio con `iniciar_pista_desde_con_settings` emite `"play"`, que registra otra vez.

Decisión: no cambiar el comportamiento, porque Node duplica y la regla es preservar el comportamiento existente. No se usó el marcador `"reinicio"`.

Prueba añadida: `corte_prematuro_registra_dos_veces_como_node` en `rust/crates/bot/src/api/eventos_reproduccion_pruebas_origenes.rs`. Afirma dos llamadas al registrador con el título "A". Pasa.

## Gates

Ejecutados en `rust/` con el toolchain 1.99.1 de `%USERPROFILE%\.cargo\bin`:

| Gate | Resultado |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 |
| `cargo test --workspace --locked` | exit 0. 348 pasan, 0 fallan |
| `cargo build --release --locked` | exit 0 |
| `npm test` (raíz) | exit 0. 234 pruebas: 227 pasan, 0 fallan, 7 omitidas |

## Archivos tocados

- `rust/crates/voice/src/admission.rs`
- `rust/crates/voice/src/admission_pruebas.rs` (nuevo)
- `rust/crates/bot/src/api/rutas/voz.rs`
- `rust/crates/bot/src/api/rutas/voz_validacion.rs` (nuevo)
- `rust/crates/bot/src/api/eventos_reproduccion_pruebas_origenes.rs`

No se tocó `rust/crates/player/src/eventos_cola.rs`: el comportamiento se conserva.

## Sin verificar

- POST /play real por el router (parte B).
- Discord, yt-dlp y reproducción audible.
- El conteo de 348 pruebas frente a 343 del informe R1 se midió en esta corrida; no hay línea base previa fiable para explicar la diferencia.
