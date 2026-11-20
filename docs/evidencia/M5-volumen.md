# M5: volumen por defecto igual que Node (100)

## Cambios

- `rust/crates/player/src/sesiones.rs`: `DEFAULT_VOLUME` pasa de 80 a 100. Es la única fuente del valor.
- `rust/crates/player/src/cola.rs`: `Cola::nueva` usa `crate::sesiones::DEFAULT_VOLUME` en vez de el literal 80.
- `rust/crates/player/tests/driver_unit.rs`: la aserción de volumen tras error usa `player::DEFAULT_VOLUME`.
- `rust/crates/player/tests/volumen_defecto.rs` (nuevo): comprueba que la cola nueva usa el valor de `Config` (`bot.default_volume`, 100 sin env).

Sesiones restauradas (`sesiones_restaurar.rs:119`) y volumen 0 no cambian.

## Pruebas y gates (ejecutados en esta sesión)

| Gate | Resultado |
|---|---|
| `cargo fmt --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| `cargo test --workspace` | 353 pasan, 0 fallan, 2 ignoradas |
| `cargo build --release --workspace` | exit 0 |
| `npm test` | 235 total, 228 pasan, 0 fallan, 7 omitidas |

Tamaño: `cola.rs` 190 líneas, `sesiones.rs` 247 líneas, ambos bajo 250.

## Decisiones

- `bot/tests/rutas_v1.rs:105` no cambia. Espera 80 en el estado de reproductor vacío, y ese valor
  viene de `estado_vacio(80)` en `bot/src/api/rutas/reproductor.rs:50`, fijado por el contrato
  compartido `test/fixtures/api-contract/player-empty.json` (`"volume": 80`).
- `bot/tests/panel_lifecycle.rs:89` y `bot/tests/panel_paridad.rs:35` no cambian. Son literales de una
  instantánea de panel, no dependen del valor por defecto.
- `bot/src/api/rutas/reproductor.rs:50` no cambia (otro archivo, fuera del alcance de M5).

## No hecho

- `DEFAULT_VOLUME` de entorno no llega a la cola. `Config` ya lo lee (1 a 100, 100 por defecto), pero
  `GestorColas::nuevo()` se llama en `bot/src/main.rs:97` sin configuración. Cablearlo exige tocar
  archivos fuera del alcance. Por eso no hay prueba con `DEFAULT_VOLUME` distinto.
- No comprobé la función `int()` de Node frente a `get_env_int` de Rust en casos límite.
- No se probó con Discord real.
- Sin commit, push, stash ni reset.
