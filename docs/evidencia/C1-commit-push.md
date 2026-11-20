# C1: commit por temas y push de feature/rust-migration

Rama: `feature/rust-migration`. Remoto: `origin` (https://github.com/DeibydBarragan/ignite-music.git).
Base: `9d5686d`. Se usó `git push -u` sin force, sin `--no-verify`, sin stash, reset ni clean.

## Commits

| SHA | Mensaje | Archivos |
|---|---|---|
| `ef668a7` | feat(rust): add Rust bot workspace crates, tests and parity scripts | 352 |
| `01f8ad4` | ci(rust): add Rust workflow and ignore rules for test env and fixtures | 4 |
| `ccf6ec0` | fix(api): align Node API guards, discovery and OpenAPI contract with Rust migration | 39 |
| `ff1fa6d` | docs(rust): add migration plan, specs, audits and verification evidence | 61 |

Cubre `rust/`, `scripts/*-paridad-ref.js`, `start-rust.bat`, `.github/workflows/rust.yml`, `.gitignore`, `eslint.config.js`, `.env.test.example`, `src/`, `test/`, `openapi.yaml` y `docs/`.

`src/` no estaba en la lista del brief. Se incluyó porque los tests de `test/` importan de `src/` (por ejemplo `discovery-ops.test.js`, `player-state-ops.test.js`) y varios specs lo citan. Sin `src/`, el commit de tests no pasaría.

## Revisión antes de commitear

- Secretos: se escaneó el contenido de los 456 archivos pendientes. Las coincidencias son nombres de variables, referencias `%VAR%` en `.bat`, o valores de prueba en fixtures (`tests/data/env-casos/caso-05-supabase-shards.env`, `test/discovery-catalog.test.js`). `.env.test.example` tiene solo nombres, sin valores. No se imprimió ningún valor.
- Excluidos por `.gitignore` y por no aparecer en `git status`: `rust/target`, `node_modules`, `data/`, logs, binarios.
- Ningún archivo pendiente pesa más de 1 MB.
- El respaldo `backup-ignite-music-2026-11-19` está fuera del repo y no se tocó.

## Estado final

- `git status --short --branch`: `## feature/rust-migration...origin/feature/rust-migration`, sin cambios pendientes.
- `git ls-remote origin feature/rust-migration` antes de este archivo: `ff1fa6d18cf11c76c8d2703398509e04b8750bfe refs/heads/feature/rust-migration`.

## Gates (tras el commit `ff1fa6d`)

| Gate | Resultado |
|---|---|
| `cargo fmt --check` (rust/) | exit 0 |
| `cargo clippy --all-targets -- -D warnings` (rust/) | exit 0 |
| `cargo test --workspace` (rust/) | exit 0, todos los bloques `test result: ok`, 0 fallos |
| `cargo build --release` (rust/) | exit 0 |
| `npm run check` | OK: 180 files |
| `npm run lint` | exit 0, sin salida |
| `npm run typecheck` | exit 0, sin salida |
| `npm test` | 235 tests, 228 pass, 0 fail, 7 skip |

Cargo se ejecutó desde `%USERPROFILE%\.cargo\bin` (cargo 1.99.1). No se corrió con la toolchain 1.89.0 instalada.

## No verificado

Discord real, yt-dlp, Supabase real y Songbird no se probaron. Este commit no cambia comportamiento de runtime; solo registra código, docs y CI.
