# Verificación independiente R2: F1, F2, F3 y estado global

Fecha: 2026-11-19. Rama `feature/rust-migration`. HEAD `9d5686d`. `git stash list` vacío. Cambios sin commit (rust/ y docs/ sin seguimiento).

No se editó código, pruebas, specs ni configuración. No hubo commit, stash, reset ni checkout. No se instaló nada. Como respaldo para comparar se usó `C:/Users/Usuario/Documents/Proyectos/ignite/backup-ignite-music-2026-11-19` con `git diff --no-index`.

## Veredictos

| Tarea | Veredicto | Motivo corto |
|---|---|---|
| F1 conservar sesión si falla el arranque | Sostenida | La sesión se conserva, no hay bucle de reintentos y caduca a los 30 min. Queda un riesgo menor (ver riesgos). |
| F2 apagado sin esperar servicios | Sostenida | Las sesiones arrancan al instante. Los servicios tienen plazo de 6 s. El código de salida no cambia. |
| F3 A. partir archivos de más de 250 líneas | Sostenida | Ningún archivo de producción supera 250 líneas. La validación de voz se movió sin cambios de texto ni de orden. |
| F3 B. prueba real de POST /play | No cubierta | La prueba engañosa se eliminó. No hay prueba que afirme el registro de historial por el router. |
| F3 C. historial en corte prematuro | Sostenida | Node registra dos veces (`events.js:102`, `119-122`). Rust hace lo mismo y lo prueba. |
| F3 en conjunto | Parcial | El registro de POST /play cambió de momento y el cambio no está en el informe de F3 (ver riesgos). |

## F1. Sesión conservada si falla el arranque

Archivo: `rust/crates/player/src/sesiones_restaurar.rs`.

- El borrado solo ocurre si el estado no es `ArranqueFallido` y la cola no está activa: línea 63-64.
- Un arranque fallido devuelve `ArranqueFallido` y deja el archivo: líneas 143-151.
- Canal de voz borrado o sin oyentes: líneas 95-98, devuelve `Descartada`, y el archivo se borra. No hay prueba específica para el canal borrado; sí para "sin oyentes" (`rust/crates/player/tests/sesiones.rs:318`).
- Caducidad: `sesiones.rs:9` (30 min) y `sesiones.rs:173-179`. `savedAt` se fija al guardar (`sesiones.rs:157`) y el reinicio no lo renueva, así que la sesión caduca 30 min después del último guardado.
- Bucle de reintentos: no hay bucle dentro de un arranque. Cada arranque intenta una vez por servidor. Con fallo permanente, la sesión caduca sola.
- Pruebas en `rust/crates/player/tests/sesiones.rs`: `:407` (arranque fallido, reinicio con éxito, pista en posición 40), `:439` (éxito sin borrado), `:458` (caducada se descarta y se borra). Afirman el estado real del archivo en disco y el seek. Son pruebas válidas.
- Riesgo no cubierto: `restauracion.rs:109-111` une el bot al canal antes de arrancar la pista. Si `iniciar_pista_desde_con_settings` falla (`restauracion.rs:114-124`), el bot queda conectado al canal sin cola. No hay `leave` en ese camino. No comparé `restauracion.rs` con el respaldo, así que no sé si ya existía.

## F2. Apagado

Archivos: `rust/crates/bot/src/arranque.rs`, `apagado.rs`, `api/ciclo_vida.rs`.

- Las sesiones y el resto de pasos corren en paralelo con los servicios: `arranque.rs:103` (`tokio::join!`) y `arranque.rs:128-130`.
- Servicios con plazo de 6 s: `arranque.rs:75` y `:93`. Si un servicio se cuelga, el futuro se corta y el guardado sigue. Verificado por lectura y por `tests/apagado_orden.rs:41-55`.
- Plazo total de 10 s: `apagado.rs:8` y `arranque.rs:128`.
- Código de salida: en el respaldo, `supervisar_gateway` devuelve `Ok(codigo) if !fallo_gateway => codigo` y `_ => 1`. Esa línea no cambió. `ejecutar_apagado` siempre devuelve 0 salvo que lo interrumpa el timeout externo. Esto ya era así (`apagado.rs:41-50`). Tiene un efecto: un apagado que agota el plazo interno sale con 0.
- Cierre del servidor API acotado: `api/ciclo_vida.rs:14` (2 s) y `:193-210`. Prueba: `tests/sse_payload.rs:176` (cliente SSE parado, termina antes de 10 s).
- Doble cierre: `hub_sse.cerrar()` se llama en `ciclo_vida.rs:182` y en `arranque.rs:125`. Es idempotente (`api/sse.rs:229-234`: limpia el mapa, marca cerrado y notifica), así que no hay daño.
- Carrera gateway y API: el gateway recibe el cierre en `cerrar_gateway` (`apagado.rs:101`) mientras la API se apaga. Un `POST /play` en esa ventana puede fallar. El worker lo admite como `shortcut:` en `arranque.rs:79-80`. Acepto que es un riesgo conocido, no un fallo de prueba.
- Pruebas: `tests/apagado_orden.rs` usa dobles de `PasosApagado`. Prueban el orden de los pasos y que el guardado llega aunque el túnel se cuelgue. No prueban la escritura real de sesiones en disco durante el apagado.

## F3. Límite de 250 líneas, POST /play e historial

A. Límite: `admission.rs` pasó de 252 a 147 líneas y `api/rutas/voz.rs` de 252 a 175. Las pruebas de `admission.rs` están en `admission_pruebas.rs` con `#[path]`. La validación de `voz.rs` está en `rutas/voz_validacion.rs`. Comparé el bloque eliminado de `voz.rs` con el respaldo: los mensajes, los límites (0.4 a 2.5, 100 pulsaciones) y el orden de las comprobaciones son los mismos. No cambió el comportamiento ni la API pública.

B. POST /play real: confirmado. `post_play_no_registra_dos_veces` no existe en `rust/` (búsqueda sin resultados). Lo que falta:

- Ninguna prueba afirma que POST /play registre el historial. Las pruebas de `eventos_reproduccion_pruebas_origenes.rs` llaman al listener, no al handler.
- `tests/api_e2e_reproductor.rs:33` y `:152` ya llaman a POST /play por el router, pero no comprueban el historial. Ese archivo es el punto natural para añadir la afirmación si el proveedor de prueba expone el registro. El worker dijo que hace falta sembrar la caché y falsear la unión de voz.

C. Historial en corte prematuro: `comprobar_corte_prematuro` deja la misma canción en repetición. Eso dispara `PLAY_SONG` otra vez en Node (`src/music/events.js:102` y `:119-122`). Rust registra dos veces, igual que Node. La prueba `corte_prematuro_registra_dos_veces_como_node` (`eventos_reproduccion_pruebas_origenes.rs:85-126`) lo afirma. Es paridad, no una corrección.

Cambio de comportamiento no declarado: en el respaldo, `proveedor_reproduccion.rs` registraba la primera pista de la petición en cuanto se encolaba (20 líneas eliminadas). Ahora el registro ocurre cuando la pista empieza (`eventos_reproduccion.rs:211`). Una canción encolada detrás de otra ya no aparece en el historial hasta que suena. Node hace lo mismo, pero el informe de F3 no lo menciona como cambio. Además, el modo dev ahora se decide por cada solicitante (`eventos_reproduccion.rs:25-48`, `es_dev_user` en la línea 44). No comprobé de dónde venía el `dev_mode` de POST /play original.

## Hallazgos del informe R1

| Hallazgo | Estado |
|---|---|
| 1. Alta: un fallo al arrancar borra la sesión | Resuelto por F1 (`sesiones_restaurar.rs:63`). |
| 2. Media-alta: el presupuesto de 10 s incluye túnel y anunciador | Resuelto por F2. El guardado ya no espera a los servicios (`arranque.rs:103`). |
| 3. Media: historial duplicado al reanudar tras corte | Aceptado por decisión documentada. Node duplica igual (`events.js:102`). No es una corrección en código. El conductor debe confirmar la aceptación. |

Los demás puntos del R1 siguen abiertos: `reset_guild_settings` sin clave (`ajustes_voz.rs:230`), `anuncio.rs` como código muerto (`api/anuncio.rs:33`), y la prueba de POST /play (ver B).

## Gates

Ejecutados por mí en esta sesión, en primer plano. Cargo desde `%USERPROFILE%\.cargo\bin`.

| Gate | Resultado |
|---|---|
| `cargo fmt --check` (rust/) | exit 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 |
| `cargo test --workspace --locked` | exit 0. Suma de las líneas `test result`: 348 pasan, 0 fallan, 2 ignoradas |
| `cargo build --release --locked` | exit 0 |
| `npm run check` | exit 0. "OK: 180 files" |
| `npm run lint` | exit 0 |
| `npm run typecheck` | exit 0 |
| `npm test` | exit 0. 234 pruebas: 227 pasan, 0 fallan, 7 omitidas |
| `cargo metadata --locked --no-deps` (rust/Cargo.toml) | exit 0. Manifiesto válido con `rust-version = "1.89"` |
| YAML de `.github/workflows/rust.yml` | No verificado. No hay parser YAML instalado (PyYAML no está, y `js-yaml` no está en `node_modules`). No instalé nada. El job `msrv` está en la línea 62 y usa `cargo +1.89.0 check --locked --workspace --tests`. |

## Comprobaciones globales

- Archivos de producción `rust/**/src/*.rs`: 198 archivos. Ninguno supera 250 líneas. El mayor es `rust/crates/bot/src/interacciones_comandos.rs` con 249.
- HEAD sigue en `9d5686d`. `git stash list` vacío.
- Escaneo de `docs/evidencia/` con patrones de JWT (`eyJ...`), `gsk_`, `sk_` y `Bearer` largo: sin coincidencias.

## Riesgos, por gravedad

1. Media. Cambio del momento de registro en POST /play (ver F3). Acorde con Node, pero no declarado en F3. Decide el conductor si lo acepta.
2. Media. Sin prueba del registro en POST /play por el router (F3 B).
3. Media-baja. Bot conectado a voz tras un arranque fallido (`restauracion.rs:109-124`). Falta comprobar si ya existía en el respaldo.
4. Baja. Carrera entre el cierre del gateway y el de la API (F2). Aceptada por el worker como `shortcut:`.
5. Baja. Las pruebas de apagado usan dobles. No hay prueba de que las sesiones lleguen a disco durante un apagado real.
6. Baja. `ejecutar_apagado` sale con 0 aunque agote su plazo interno (`apagado.rs:41-50`). Ya era así antes del cambio.
7. Baja. Sesión conservada hasta 30 min después del último guardado. Pasado ese tiempo caduca.
8. Baja. `reset_guild_settings` y `anuncio.rs` del R1 siguen igual.

## Sin verificar

- Discord, Supabase, yt-dlp, túnel cloudflared, reproducción audible y autoplay.
- Apagado real con bot y sesiones escritas en disco.
- Mutaciones (quitar el defer, quitar el filtro de historial).
- Validez del YAML del workflow (sin parser disponible).
- Si el registro de historial en POST /play por el router funciona: no hay prueba que lo afirme.
