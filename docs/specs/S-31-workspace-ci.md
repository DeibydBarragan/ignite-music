# Spec S-31: Workspace de Cargo y CI

- Estado: workspace implementado, validación remota de CI pendiente
- Fase: F1 (la necesita F2)
- Módulo: M8
- Origen en Node: ninguno. Solo se añade código nuevo.

## Objetivo

Crear un workspace de Rust que compila en Windows, pasa `fmt` y `clippy`, y corre en CI. No tiene lógica de bot.

## Decisiones

- Ubicación: `ignite-music/rust/`, dentro de la rama `feature/rust-migration`.
- Rust 1.99.1 es la versión soportada para el workspace y la compilación de release. `rust-toolchain.toml` la fija y los comandos con `--locked` usan las versiones de `Cargo.lock`.
- Target: `x86_64-pc-windows-msvc`. Songbird compila Opus con CMake en Windows (README de songbird), y CMake usa Visual Studio por defecto en Windows. El bot ya carga `libvosk.dll` desde Node con koffi. Esa DLL trae runtime de MinGW, así que cargarla desde un proceso MSVC ya funciona hoy.
- Plan B: `x86_64-pc-windows-gnu`.
- Cargo usa CMake desde el PATH o desde la ruta indicada mediante `CMAKE` en el entorno. La configuración del repositorio fija `CMAKE_POLICY_VERSION_MINIMUM = "3.5"` para compilar Opus con CMake 4.

## Requisitos de la máquina

Instalados el 2026-11-07 con winget. Verificado: `cl` y `link` responden.

- Visual Studio Build Tools 2022 (17.14.44), con la carga de trabajo de C++. Ruta: `C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools`. MSVC 14.44.35207 y SDK de Windows 10.0.26100.0.
- CMake disponible en el PATH o mediante `CMAKE`.

## Archivos

- Crear: `rust/Cargo.toml` (workspace, `resolver = "2"`, miembros `crates/*`)
- Crear: `rust/rust-toolchain.toml`
- Crear: `rust/.cargo/config.toml` con la política de CMake
- Crear: `rust/.gitignore` (`/target`)
- Crear: `rust/crates/voice/` (biblioteca vacía)
- Crear: `rust/crates/player/` (biblioteca vacía)
- Crear: `rust/crates/bot/` (binario con `fn main() {}`)
- Crear: `rust/crates/medios/` (biblioteca vacía, M3)
- Crear: `.github/workflows/rust.yml` (job en `windows-latest`)
- No tocar: `src/`, `test/`, `package.json` ni los workflows que ya existen.

## Comandos

- `cargo fmt --all --check`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace --no-fail-fast`
- `cargo build --locked --release -p bot`
- Node, sin cambios: `npm run check`, `npm run lint`, `npm run typecheck`, `npm test`.

## Tests

Verificar los comandos anteriores desde una copia que incluya los fixtures sintéticos de `rust/crates/*/tests/data/`. El workflow informa si tiene acceso a los modelos y DLL de Vosk y a ffmpeg, porque parte de la cobertura de audio depende de esos recursos.

## Criterio de aceptación

- `cargo build` compila el workspace en Windows con el target MSVC.
- Los comandos de Rust pasan en local.
- Opus se compila con un CMake disponible en el entorno sin rutas personales en el repositorio.
- El workflow pasa en GitHub cuando se suba la rama.
- Los comandos de Node pasan igual que antes.
- `git status` no muestra cambios en `src/` ni en `test/`.

## Pendiente de validación

- Correr el workflow de Windows en GitHub con los archivos del workspace y los fixtures incluidos en la rama.
