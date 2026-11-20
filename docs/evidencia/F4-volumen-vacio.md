# F4: volumen del estado vacío y límite de proveedor_real.rs

Fecha: 2026-11-19. Rama feature/rust-migration. Sin commit, push, stash ni reset.

## Cambios

- `rust/crates/bot/src/api/rutas/reproductor.rs`: el estado sin cola usa `player::DEFAULT_VOLUME` (100) en lugar de 80.
- `test/fixtures/api-contract/player-empty.json`: `volume` pasa de 80 a 100.
- `rust/crates/bot/tests/fixtures/rutas-node.json`: `player_state_empty.volume` pasa de 80 a 100 (la prueba `test_node_fixtures_parity` falló sin este cambio).
- `rust/crates/bot/tests/rutas_v1.rs`: la aserción de `volume` del estado vacío pasa de 80 a 100.
- `rust/crates/bot/src/api/proveedor_real_aux.rs`: nueva función `extension_icono(hash)` con la lógica gif/webp que estaba en `proveedor_real.rs`.
- `rust/crates/bot/src/api/proveedor_real.rs`: usa `extension_icono`. Pasa de 251 a 247 líneas. El seam de M6 no se tocó.

## Node y el valor por defecto

- `test/api.test.js` lee `player-empty.json`, pero mockea la operación y devuelve la fixture tal cual. Pasa con 80 o con 100.
- Node devuelve `config.bot.defaultVolume` (100 por defecto) en `src/api/v1/ops/player.js:28`.

## musica.rs:126 (`value.unwrap_or(80)`): sin cambios

Node no usa `defaultVolume` en el control de volumen. `src/voice/dispatcher.js:209` hace `Number.parseInt(query, 10)`, y sin valor da NaN. Según `test/fixtures/api-contract/player-controls.json`, Node responde con error «Volume must be a number between 0-100!» cuando falta el valor. Rust, en cambio, aplica 80 cuando falta. Esa diferencia es un hueco distinto del de este item y queda sin resolver.

## Resultados

| Gate | Resultado |
|---|---|
| `cargo fmt --all -- --check` | pasa (exit 0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | pasa |
| `cargo test --workspace --no-fail-fast` | 356 pasan, 0 fallan, 2 ignoradas (86 binarios) |
| `cargo build --release --workspace` | pasa |
| `npm test` | 228 pasan, 0 fallan, 7 omitidas |
| Archivos de producción `.rs` > 250 líneas | ninguno |

Las pruebas de integración en `tests/` superan 250 líneas. No cuentan como código de producción según la indicación, así que no se recortaron.

## No verificado

- La función `extension_icono` no tiene prueba propia. Es una rama simple, cubierta solo de forma indirecta por las pruebas de servidores.
- El recuento de `npm test` es 228, no 227 como en los reportes anteriores. No se investigó la causa.
- No se revisó el comportamiento en Discord real.
