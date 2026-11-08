# Spec S-31: Workspace de Cargo y CI

- Estado: borrador
- Fase: F1 (la necesita F2)
- Módulo: M8
- Origen en Node: ninguno. Solo se añade código nuevo.

## Objetivo

Crear un workspace de Rust que compila en Windows, pasa `fmt` y `clippy`, y corre en CI. No tiene lógica de bot.

## Decisiones

- Ubicación: `ignite-music/rust/`, dentro de la rama `feature/rust-migration`.
- Toolchain: stable, con versión mínima 1.83. Es el MSRV de songbird 0.6.0. Se fija en `rust-toolchain.toml`.
- Target: `x86_64-pc-windows-msvc`. Songbird compila Opus con CMake en Windows (README de songbird), y CMake usa Visual Studio por defecto en Windows. El bot ya carga `libvosk.dll` desde Node con koffi. Esa DLL trae runtime de MinGW, así que cargarla desde un proceso MSVC ya funciona hoy.
- Plan B: `x86_64-pc-windows-gnu`.
- CMake: 4.4.4, solo para este proyecto. Se fija con la variable `CMAKE` en `rust/.cargo/config.toml`, sección `[env]`. El PATH del usuario no cambia. La ruta es de esta máquina, así que el workflow de CI define `CMAKE` como `cmake`.

## Requisitos de la máquina

Instalados el 2026-11-07 con winget. Verificado: `cl` y `link` responden.

- Visual Studio Build Tools 2022 (17.14.44), con la carga de trabajo de C++. Ruta: `C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools`. MSVC 14.44.35207 y SDK de Windows 10.0.26100.0.
- CMake 4.4.4 (`Kitware.CMake`). Alias: `C:\Users\Usuario\AppData\Local\Microsoft\WinGet\Links\cmake.exe`.
- `cmake` en el PATH: hoy gana el 4.0.3 de WinLibs. El proyecto no lo usa, ver Decisiones.

## Archivos

- Crear: `rust/Cargo.toml` (workspace, `resolver = "2"`, miembros `crates/*`)
- Crear: `rust/rust-toolchain.toml`
- Crear: `rust/.cargo/config.toml` (`[env]` con `CMAKE`, ver Decisiones)
- Crear: `rust/.gitignore` (`/target`)
- Crear: `rust/crates/voice/` (biblioteca vacía)
- Crear: `rust/crates/player/` (biblioteca vacía)
- Crear: `rust/crates/bot/` (binario con `fn main() {}`)
- Crear: `.github/workflows/rust.yml` (job en `windows-latest`)
- No tocar: `src/`, `test/`, `package.json` ni los workflows que ya existen.

## Comandos

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo build -p bot`
- Node, sin cambios: `npm run check`, `npm run lint`, `npm run typecheck`, `npm test`.

## Tests

Esta spec no tiene tests de lógica. Los comandos de arriba son la prueba.

## Criterio de aceptación

- `cargo build` compila el workspace en Windows con el target MSVC.
- Los comandos de Rust pasan en local.
- Opus se compila con CMake 4.4.4, sin cambiar el PATH del usuario.
- El workflow pasa en GitHub cuando se suba la rama.
- Los comandos de Node pasan igual que antes.
- `git status` no muestra cambios en `src/` ni en `test/`.

## Preguntas abiertas

- Decidido: CMake 4.4.4 (ver Decisiones).
- Si Opus falla con "Compatibility with CMake < 3.5 has been removed", añadir `CMAKE_POLICY_VERSION_MINIMUM = "3.5"` al mismo `[env]`. CMake 4.0 quitó esa compatibilidad. Probar en el primer build.
- ¿Trae CMake el runner `windows-latest`? Verificar en el primer run del workflow.
