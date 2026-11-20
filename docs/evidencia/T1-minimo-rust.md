# T1: mínimo de Rust del workspace

Estado: hecho. El mínimo real es **1.89.0**. `cargo +1.89.0 check --locked --workspace --tests` pasa.

## Cómo se calculó

- Se leyó `rust-version` de cada paquete con `cargo metadata --locked --format-version 1` (492 paquetes).
- Dependencias externas: el valor más alto es 1.89 (y 1.89.0 en `uuid 1.29.1`). Entre ellas están `twilight-*` 0.17 (1.89), `tokio-websockets` 0.13.3 (1.89) y `twilight-util` 0.17.0 (1.89).
- Crates propios (`bot`, `nucleo`, `player`, `medios`, `voice`): heredan `rust-version.workspace = true`, así que no fijan un mínimo propio.
- El código propio también compila con 1.89.0, lo que confirma la prueba de `check`.

## Decisión

- `rust/Cargo.toml`: `rust-version = "1.89"` (antes `"1.99.1"`).
- `rust/rust-toolchain.toml`: sin cambios, sigue en `1.99.1` como versión de desarrollo.
- `.github/workflows/rust.yml`: nuevo job `msrv` que instala `1.89.0` con `rustup toolchain install 1.89.0 --profile minimal` y corre `cargo +1.89.0 check --locked --workspace --tests` en `rust/`.

## Archivos tocados

- `rust/Cargo.toml` (solo la línea `rust-version` y su comentario).
- `.github/workflows/rust.yml` (job `msrv` añadido al final).
- `docs/evidencia/T1-minimo-rust.md` (este archivo).
- Respaldo previo: `C:/Users/Usuario/Documents/Proyectos/ignite/backup-ignite-music-2026-11-19/` (`rust/Cargo.toml`, `rust/rust-toolchain.toml`, `rust/Cargo.lock`, `.github/workflows/rust.yml`).
- No se tocaron `Cargo.lock`, dependencias, ni otros archivos del trabajo sin commit.

## Comandos y resultados

| Comando | Resultado |
|---|---|
| `rustup toolchain install 1.89.0 --profile minimal` | Instalado: rustc 1.89.0 (29483883e 2025-08-04). |
| `cargo +1.89.0 check --workspace --locked` (en `rust/`) | Pasa (exit 0). Solo avisos de `unused_imports` en `player` y `unused_variables` en `bot` (`cola.rs`). |
| `cargo +1.89.0 check --locked --workspace --tests` (en `rust/`) | Pasa (exit 0). |
| `cargo fmt --all --check` (en `rust/`) | Pasa (exit 0). |
| `cargo clippy --all-targets --locked -- -D warnings` | Pasa (exit 0). |
| `cargo test --workspace --locked --no-fail-fast` | Pasa. Todos los bloques `test result: ok`, sin fallos. |
| `cargo build --release --locked` | Pasa (exit 0). |
| `npm test` (raíz) | 234 pruebas: 227 pasan, 0 fallan, 7 omitidas. |
| Parseo YAML de `rust.yml` | No verificado. No hay parser YAML instalado (ni PyYAML, ni `yaml`/`js-yaml` global o local). Revisión visual: la indentación del job `msrv` coincide con la del job `verify`. |

## Riesgos

- Los avisos de 1.89.0 (`unused_imports`, `unused_variables`) no bloquean `check`, pero el job `msrv` no usa `-D warnings`.
- El YAML no se validó con un parser. Un error de sintaxis lo vería la primera ejecución de CI.
