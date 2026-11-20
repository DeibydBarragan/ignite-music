# Verificación R3: M1, M3, M5, M6, M7 y gates

Verificador independiente. Solo lectura y gates. Sin commit, push, stash ni reset.
Estado: HEAD `9d5686d`. `git stash list` vacío. El árbol tiene cambios sin commit, que no se tocaron.
Base de comparación: `backup-ignite-music-2026-11-19`, copia hecha antes de esta ronda. `rust/crates/bot` no está bajo git.

## Veredictos

| Punto | Veredicto | Motivo corto |
|---|---|---|
| M1 | Sostenida | Sin `\\d` en `openapi.yaml`. Prueba nueva detecta doble barra (probado). |
| M3 | Sostenida | Sin clave: cero HTTP, 200 con defecto, `warn!`. Resto de escrituras sin cambio de M3. |
| M5 | Parcial | Cola nueva = 100 desde una fuente. Estado vacío sigue en 80, distinto de Node. |
| M6 | Sostenida | Pruebas llaman a `post_play`. Seam apagado en producción. Archivo sobre 250 líneas. |
| M7 | Sostenida | `salir_de_voz` tras `ArranqueFallido`. Sesión conservada. Prueba con proveedor falso. |
| M2 | Premisa confirmada | `rust.yml` parsea. Job `msrv` usa 1.89.0. |
| M4 | Premisa falsa | `api/anuncio.rs` está vivo. |

## M1: patrón de ID en `openapi.yaml`

- `openapi.yaml`: 13 patrones, todos con `\d`. Ninguno con `\\`. Líneas 72, 73, 216, 264, 395, 562, 563, 632, 638, 683, 689, 702, 792.
- `test/openapi.test.js:72-82`: el test recorre todos los `pattern:`. La línea 75 falla si un patrón contiene `\\`.
- Prueba de la lógica del test con el patrón antiguo `'^\\d{17,20}$'`: marcado. Con `'^\d{17,20}$'`: no marcado.
- Evidencia: `docs/evidencia/M1-openapi-patron.md`. Sus líneas 562, 563, 683 y 792 coinciden con el archivo.

## M3: reinicio de ajustes de voz

- `rust/crates/bot/src/datos/ajustes_voz.rs:225-243`: `reset_guild_settings`.
  - Línea 231: `c.delete(&ruta)` devuelve `Err` sin clave de servicio. No se abre conexión.
  - Línea 237: ese `Err` queda registrado con `warn!`. Cada `DELETE` deja un aviso.
  - Línea 242: devuelve los valores por defecto.
- `rust/crates/bot/src/api/almacen_voz.rs:105-108` y `rust/crates/bot/src/api/rutas/voz.rs:70-71`: la ruta responde 200 con el valor devuelto.
- `rust/crates/bot/src/datos/supabase.rs:114-127`: `post`, `patch` y `delete` exigen clave de escritura. Esa parte viene de T4, no de M3.
- Diferencia con el backup en `ajustes_voz.rs`: el cambio de reinicio es de M3. Los cambios en `update_guild_settings` (líneas 168-216) son de T4.
- Pruebas: `rust/crates/bot/tests/ajustes_voz_reset.rs`, 4 pruebas, pasan. Sin clave: no abre conexión. Con clave: `DELETE` con `Bearer` de servicio. Error HTTP y conexión caída: valores por defecto.
- Hueco: ninguna prueba mira el texto del `warn!`. El aviso no está verificado.
- Evidencia: `docs/evidencia/M3-reset-ajustes.md`. Sus cifras coinciden.

## M5: volumen por defecto

- Cola nueva: `rust/crates/player/src/sesiones.rs:12` define `DEFAULT_VOLUME = 100`. `rust/crates/player/src/cola.rs:99` lo usa. Es la única fuente.
- Sesiones restauradas: `rust/crates/player/src/sesiones_restaurar.rs:125` asigna `data.volume` directamente. El volumen 0 se conserva.
- Quedó en 80:
  - `rust/crates/bot/src/api/rutas/reproductor.rs:50`: `estado_vacio(80, false)`. Es el estado sin cola de `GET /player` y SSE.
  - `rust/crates/bot/tests/rutas_v1.rs:105`: espera 80 y compara con `test/fixtures/api-contract/player-empty.json` (`"volume": 80`).
  - `rust/crates/player/src/musica.rs:126`: `value.unwrap_or(80)` en la acción `volume` sin valor.
- Coherencia con Node: no coincide en el estado vacío. `src/api/v1/ops/player.js:28` devuelve `config.bot.defaultVolume`, 100 por defecto. Rust devuelve 80. El test de Node (`test/api.test.js:135-150`) simula la operación, así que no detecta la diferencia.
- `DEFAULT_VOLUME` de entorno no llega a la cola. `rust/crates/nucleo/src/config.rs:106` lo lee, pero `sesiones.rs:12` es constante.
- Evidencia: `docs/evidencia/M5-volumen.md`. Dice que el estado vacío "viene de" el contrato. El contrato refleja Rust, no Node.

## M6: historial de POST /play

- Pruebas: `rust/crates/bot/src/api/post_play_historial_pruebas.rs:201-229`. Llaman a `post_play` (línea 173 y 8). Pasan.
- Campos de inyección: `rust/crates/bot/src/api/proveedor_real.rs:29` (`prueba_resolucion`) y `:31` (`prueba_sin_union`). Valor por defecto en `:64-65`: `None` y `false`.
- Uso en producción: `rust/crates/bot/src/api/proveedor_reproduccion.rs:27` (`if prov.prueba_sin_union`) y `:41` (`if let Some(falsa)`). Con los valores por defecto, la rama real se ejecuta: `conectar_canal_voz` en `:33`, resolución por yt-dlp en las ramas siguientes.
- Ningún camino de producción activa los campos. Búsqueda en todo `rust/`: solo `post_play_historial_pruebas.rs:148-149` los asigna. El módulo de prueba se monta con `#[cfg(test)]` (`proveedor_reproduccion.rs:216-218`).
- Hueco: `proveedor_real.rs` tiene 251 líneas. Línea 251 sobre el límite de 250. La copia de backup tenía 244. La evidencia M6 dice 232, así que está desactualizada.
- Evidencia: `docs/evidencia/M6-historial-post-play.md`.

## M7: salir del canal si falla el arranque

- `rust/crates/player/src/sesiones_restaurar.rs:61-63`: si el estado es `ArranqueFallido`, llama a `salir_de_voz`. Se llama después de soltar el bloqueo de la cola (`restaurar_servidor` ya terminó).
- `sesiones_restaurar.rs:69-71`: la sesión solo se borra si el estado no es `ArranqueFallido`. La sesión queda en disco.
- `rust/crates/bot/src/restauracion.rs:131-138`: `salir_de_voz` llama a `drv.leave(gid)`.
- Otras salidas de la restauración: `arrancar_reproduccion` (`restauracion.rs:101-128`) solo devuelve `Ok` cuando la pista arranca. Cualquier error llega a `ArranqueFallido`. Los `Descartada` salen antes de `drv.join`.
- Pruebas: `rust/crates/player/tests/sesiones.rs:500-534`. El proveedor falso registra la salida de `guild_union`. Pasa.
- Hueco: la prueba usa un proveedor falso. `drv.leave` contra Songbird no se prueba. Un `panic` dentro de `arrancar_reproduccion` no pasaría por este camino. No se encontró otra ruta que deje al bot unido.
- Evidencia: `docs/evidencia/M7-salir-de-voz.md`.

## M2 y M4

- M2: `.github/workflows/rust.yml` parsea con `js-yaml` de `ignite-frontend/node_modules`. Jobs: `verify`, `msrv`. El job `msrv` instala `1.89.0` (línea 71) y ejecuta `cargo +1.89.0 check` (línea 73). `rust/Cargo.toml` fija `rust-version = "1.89"`.
- M4: `rust/crates/bot/src/api/anuncio.rs` no está abandonado. `ciclo_vida.rs:153` crea `Anunciador::nuevo` y `ciclo_vida.rs:176` lo usa. `api/mod.rs:27` lo exporta.

## Gates (ejecutados en esta ronda)

| Gate | Resultado |
|---|---|
| `cargo fmt --all -- --check` | Sin diferencias, exit 0 |
| `cargo clippy --all-targets -- -D warnings` | Terminó sin avisos, exit 0. Caché de la ronda anterior (1,29 s) |
| `cargo test --workspace` | 356 pasan, 0 fallan, 2 ignoradas |
| `cargo build --release --workspace` | Terminó, exit 0 |
| `npm run check` | OK: 180 archivos |
| `npm run lint` | Sin errores |
| `npm run typecheck` | Sin errores |
| `npm test` | 228 pasan, 0 fallan, 7 omitidas |

Longitud de archivos de producción (`rust/crates/*/src`): un archivo supera 250 líneas.

| Archivo | Líneas |
|---|---|
| `rust/crates/bot/src/api/proveedor_real.rs` | 251 |

## Riesgos no incluidos en los informes

1. `proveedor_real.rs` supera el límite de 250 líneas. Lo causa el seam de M6 (+7 líneas). La evidencia M6 dice 232.
2. El reinicio sin clave de servicio no borra en Supabase. Cierra la caché (`ajustes_voz.rs:241`). La siguiente lectura (`ajustes_voz.rs:50-70`) vuelve a leer la base y muestra los ajustes antiguos. El usuario ve valores por defecto solo una vez.
3. El estado de reproductor vacío difiere de Node (80 frente a 100). El contrato compartido lo fija en 80. Ninguna prueba de Node ve el valor real.
4. `DEFAULT_VOLUME` de entorno no tiene efecto en las colas de Rust.
5. Los cambios sin commit de `rust/` no están en git. La comparación se hizo con el backup del 2026-11-19. El backup ya incluye cambios de T4 en `ajustes_voz.rs` y `supabase.rs`.
6. El texto del `warn!` de M3 y de M7 no se prueba.

## No verificado

- Discord, Songbird y yt-dlp reales.
- Supabase real.
- Valor de volumen cuando una sesión guardada no trae el campo `volume`.
- Ejecución de `cargo +1.89.0 check`. El toolchain no está instalado en esta máquina.
