# Verificación independiente R1: T2, T3, T4, T5, T6, T8

Fecha: 2026-11-19. Rama `feature/rust-migration`. HEAD `9d5686d3cc171b11c7494f951700f16e6bbfc5cb`. `git stash list` vacío. No se editó código, pruebas, specs ni configuración. No hubo commit, stash, reset ni checkout.

## Veredictos

| Tarea | Veredicto | Motivo corto |
|---|---|---|
| T2 sesiones | Parcial | Código y pruebas de restauración se sostienen. Una pérdida de sesión al fallar el arranque no está en el informe. |
| T3 historial | Parcial | Registro único por "play" se sostiene. La prueba de POST /play no ejercita POST /play. Duplicado posible en recuperación por corte. |
| T4 supabase | Sostenida | Escrituras se bloquean antes de cualquier HTTP. Los cuatro llamadores propagan el error. |
| T5 gate_unavailable | Sostenida | Pruebas afirman el motivo y el conteo de llamadas a Groq. Falla la regla de 250 líneas. |
| T6 defer /play | Sostenida | Las dos pruebas fallarían sin el defer. No se hizo prueba de mutación. |
| T8 SSE | Parcial | Paridad y cierre acotado se sostienen. El plazo de 10 s compartido puede cortar el guardado. |

## Evidencia por tarea

### T2 sesiones

- Plugins `direct` y `generic` añadidos: `rust/crates/player/src/sesiones.rs:78`.
- `initialize` al arrancar: `rust/crates/bot/src/main.rs:100`. Y dentro de la restauración: `rust/crates/player/src/sesiones_restaurar.rs:44`.
- Volumen 0 conservado: `sesiones_restaurar.rs:110`. La prueba `test_restaura_sesion_escrita_por_node_con_todos_los_tipos` (`rust/crates/player/tests/sesiones.rs:280-316`) afirma volumen 0, pausa, filtros, posición 40 y el arranque `("guild_mixta", Some(40))`. Es una afirmación real.
- Fixture `rust/crates/player/tests/data/sesiones/guild_mixta.json`: contiene las cuatro fuentes y no tiene secretos.
- `test_restaurar_sin_initialize_lee_el_disco` (`sesiones.rs:345`) sí prueba algo, porque `restaurar_sesiones` llama `initialize` por sí mismo.
- Límites que el propio informe admite y que confirmo: no hay guardado periódico, los bots cuentan distinto que en Node y no se comprueba `joinable`.
- Riesgo no reportado: si `arrancar_reproduccion` falla (yt-dlp, unión a voz), `restaurar_servidor` deja la cola inactiva y `restaurar_sesiones` borra el archivo (`sesiones_restaurar.rs:58-62`). Con T2, el arranque ahora puede fallar por yt-dlp. Un fallo transitorio al arrancar borra la sesión guardada para siempre.

### T3 historial

- Solo el evento `"play"` registra: `rust/crates/bot/src/api/eventos_reproduccion.rs:202`, y el registro en `:211`. Se sostiene.
- Seek y filtro emiten `"reinicio"`: `rust/crates/player/src/reproduccion.rs:110-125` (`reiniciar_pista_desde_con_settings`, `reinicio = true`) y `driver_reproduccion.rs:128-138`. `rust/crates/player/tests/reinicio_historial.rs:43-100` comprueba la secuencia de eventos `["play", "reinicio", "reinicio"]`. Es real.
- Avance: `driver.rs:179` emite `"advance"`, y el listener vuelve a arrancar con `"play"`. Registra una vez por pista (`eventos_reproduccion_pruebas_origenes.rs:29-54`).
- Restauración registra historial, igual que en Node según el propio informe. Se sostiene como paridad.
- POST /play: no queda ninguna llamada a `registrar` en `rust/crates/bot/src/api/proveedor_reproduccion.rs`. La eliminación se sostiene.
- Prueba `post_play_no_registra_dos_veces` (`eventos_reproduccion_pruebas_origenes.rs:83-100`): no llama a POST /play. Llama a `iniciar_pista_con_settings`, igual que `origen_comando_registra_una_vez`. Por eso no prueba la afirmación (b) y no puede fallar por el handler HTTP. Es una prueba redundante con nombre engañoso.
- Seek y filtro no se prueban a nivel de listener. Solo se prueba la emisión de eventos en el driver.
- Autoplay: no se probó, como dice el informe. Confirmo que `eventos_reproduccion.rs` rama autoplay no tiene prueba.
- Duplicado probable: `comprobar_corte_prematuro` (`rust/crates/player/src/eventos_cola.rs`) fija `modo_repeticion = Cancion` y no quita la pista. Luego `iniciar_pista_desde_con_settings` emite `"play"` (`eventos_reproduccion.rs:202`). La misma canción queda registrada dos veces. No comprobé si Node hace lo mismo.

### T4 supabase

- `escribir` devuelve `SinCredencialEscritura` antes de crear la petición: `rust/crates/bot/src/datos/supabase.rs:101-107`. `post`, `patch` y `delete` (`:114` en adelante) pasan por ahí. Sin clave no hay HTTP.
- Lecturas con la clave de lectura: `supabase.rs:111` (`get`) y `:132` (`rpc`).
- Llamadores de escritura, todos propagan o registran el error:
  - `datos/ajustes_voz.rs:171` (503 si falla, según el informe), `:183`, `:199`, `:230` (estos tres ignoran el error, como antes).
  - `datos/favoritos.rs:91` y `:118`: 500 con código de error.
  - `datos/historial.rs:200`: `debug!`, la reproducción sigue.
  - `datos/listas.rs:104` y `:128`: 500 con código de error.
- No hay escritura con la clave anónima en producción. La única ruta con clave arbitraria es `api/anuncio.rs:33` (`ClienteSupabaseHttp`). Ningún código fuera de su definición lo construye (búsqueda en `rust/crates/bot/src`). Es código muerto que escribe con la clave que reciba.
- Pruebas: los nombres de `rust/crates/bot/tests/supabase_escritura.rs:49, 67, 79, 90` coinciden con las afirmaciones. No revisé el cuerpo de cada prueba línea a línea.
- Riesgo ya señalado en el informe: `reset_guild_settings` sin clave (`ajustes_voz.rs:230`) limpia la caché y devuelve valores por defecto. Los valores guardados en Supabase no se borran.

### T5 gate_unavailable

- Motivo `gate_unavailable` cuando no hay evaluador: `rust/crates/voice/src/admission.rs:96`.
- Prueba nueva `gate_no_disponible_no_admite_y_no_gasta_cooldown` (`admission.rs:212-236`): tras el rechazo, la misma petición con gate abierto sí se admite. Demuestra que el cooldown no se gastó. Real.
- Prueba nueva `gating_desactivado_admite_sin_evaluador` (`admission.rs:239-251`). Real.
- Conteo de Groq: `rust/crates/bot/src/comandos_voz/pruebas.rs:70-132`. Cuenta llamadas con `GroqFalso` y afirma 0 con gate no disponible (`:114`) y 1 con gate abierto (`:132`). Real.
- Fallo de regla: `rust/crates/voice/src/admission.rs` tiene 252 líneas. El informe dice "menos de 250". Es falso, y el archivo supera el límite de AGENTS.md.

### T6 defer /play

- El defer ocurre en el despacho, antes de la unión: `rust/crates/bot/src/interacciones_comandos.rs:26`. La premisa del revisor era incorrecta, como dice el informe.
- `play_responde_con_defer_antes_de_que_termine_la_union` (`rust/crates/bot/tests/defer_play.rs:216-246`): el shard sigue vivo, así que la unión espera. Si el defer no existiera, `esperar_llamada` (`:84`) no vería ningún `/callback` en 5 s y el test fallaría. Se sostiene sin ejecutar la mutación.
- `union_fallida_avisa_al_usuario_y_no_encola_la_pista` (`defer_play.rs:249-275`): exige dos llamadas con el defer primero (`:266`). Sin defer, la primera llamada sería el PATCH y la prueba fallaría. Se sostiene.

### T8 SSE

- Paridad: `rust/crates/bot/tests/sse_payload.rs:104-124` (evento inicial igual a GET /player) y `:126-149` (player.sync igual a GET /player). Ambas comparan cuerpos completos. Reales.
- `sincronizacion_nunca_emite_cuerpo_vacio` (`sse_payload.rs:152-173`): solo comprueba que el cuerpo es un objeto con `hasQueue`. Es una afirmación débil, aunque no es tautología.
- Cierre acotado: `rust/crates/bot/src/api/ciclo_vida.rs:193-211`. Envía la señal, espera 2 s y aborta. La prueba `apagado_con_cliente_sse_parado_termina_antes_de_10_s` (`sse_payload.rs:176-215`) reproduce el bloqueo con 80 eventos de 256 KiB. Real.
- Orden de apagado: `rust/crates/bot/src/arranque.rs:90-93`. Todo va dentro de un `timeout(grace = 10 s)` que incluye `servicios.apagar()` (anunciador, túnel, API de hasta 2 s) y después `ejecutar_apagado`, que guarda sesiones en `apagado.rs` (`apagar_reproductor`). El aborto de 2 s no quita sesiones por sí mismo, porque el guardado va después. Pero el tiempo de anunciador y túnel descuenta del mismo presupuesto de 10 s. Si se agota, el timeout externo cancela el guardado. El tiempo de `tunel.detener()` no se midió.
- Esta prueba no comprueba que las sesiones se guarden. Solo comprueba el plazo del servidor.

## Gates (ejecutados por mí, en primer plano)

El primer intento en segundo plano no dejó logs. Cargo no está en el PATH; lo usé desde `%USERPROFILE%\.cargo\bin`. Toolchain `1.99.1` instalado.

| Gate | Resultado |
|---|---|
| `cargo fmt --check` (rust/) | exit 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 |
| `cargo test --workspace --locked` | exit 0. Suma de todas las líneas `test result`: 343 pasan, 0 fallan, 2 ignoradas |
| `cargo build --release --locked` | exit 0 |
| `npm run check` | exit 0 |
| `npm run lint` | exit 0 |
| `npm run typecheck` | exit 0 |
| `npm test` | exit 0. 234 pruebas: 227 pasan, 0 fallan, 7 omitidas |

Las cifras de los informes de cada tarea (321 a 343) difieren porque se midieron en momentos distintos. Esta corrida da 343.

## Comprobaciones adicionales

- HEAD sigue en `9d5686d3cc171b11c7494f951700f16e6bbfc5cb`. Stash vacío.
- Archivos Rust de producción por encima de 250 líneas: `rust/crates/voice/src/admission.rs` (252, tocado por T5) y `rust/crates/bot/src/api/rutas/voz.rs` (252, no está en estas tareas; no comprobé si lo tocó alguna).
- Escaneo de `docs/evidencia/` con patrones de JWT, `gsk_`, `sk_` y `Bearer` largo: sin coincidencias.

## Riesgos, por gravedad

1. Alta. `rust/crates/player/src/sesiones_restaurar.rs:58-62`: un fallo al arrancar la pista borra la sesión guardada. Con T2 ese fallo ya es posible (yt-dlp, unión a voz). Conviene no borrar si el arranque falla.
2. Media-alta. `rust/crates/bot/src/arranque.rs:90-93`: el presupuesto de 10 s incluye anunciador, túnel y API. Un túnel lento puede cortar el guardado de sesiones. Falta prueba del apagado completo con sesiones.
3. Media. Historial duplicado al reanudar tras corte prematuro (`eventos_cola.rs`, `eventos_reproduccion.rs:202`). No verificado contra Node.
4. Media. `reset_guild_settings` sin clave de servicio (`ajustes_voz.rs:230`) responde como si hubiera borrado. Ya estaba en el informe de T4.
5. Media-baja. POST /play sin prueba real. `post_play_no_registra_dos_veces` no llama al handler.
6. Baja. `rust/crates/voice/src/admission.rs` tiene 252 líneas. Hay que partir el módulo de pruebas.
7. Baja. `rust/crates/bot/src/api/anuncio.rs:33` (`ClienteSupabaseHttp`) es código muerto que escribe con la clave que reciba. Si alguien lo conecta con la clave anónima, reabre el problema de T4.

## Sin verificar

- Discord real, Supabase real, yt-dlp, reproducción audible y autoplay.
- Mutación real (quitar el defer, quitar el filtro de historial).
- Cuerpos completos de las pruebas de T4 y de T8 (c).
- Comportamiento de Node en la recuperación por corte.
