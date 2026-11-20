# M2: validación YAML de .github/workflows/rust.yml

Fecha: 2026-11-19. Archivo: `.github/workflows/rust.yml` (sin cambios).

## Parser

No hay `yaml` en Python ni `yaml`/`js-yaml` en `ignite-music/node_modules`. Se usó `js-yaml` 4.3.2 ya instalado en `ignite-frontend/node_modules`, en modo lectura. No se instaló nada.

## Comando y resultado

```
node -e "const y=require('C:/Users/Usuario/Documents/Proyectos/ignite/ignite-frontend/node_modules/js-yaml'); ... y.load(fs.readFileSync('.github/workflows/rust.yml','utf8')) ..."
```

Resultado: el parseo termina sin error. Código de salida 0.

## Forma del workflow

- Claves de nivel superior: `name`, `on`, `permissions`, `jobs`.
- `on`: `push` y `pull_request`, ambos con `paths` (ancla YAML `&rust_paths` reutilizada con `*rust_paths`).
- Jobs:
  - `verify`: `runs-on: windows-latest`, 10 pasos.
  - `msrv`: `runs-on: windows-latest`, 3 pasos. Comandos: `rustup toolchain install 1.89.0 --profile minimal` y `cargo +1.89.0 check --locked --workspace --tests`.

## Límites

- Solo se validó la sintaxis YAML y la forma del documento. No se validó el esquema de GitHub Actions ni se ejecutó el workflow.
- `js-yaml` sigue YAML 1.2. GitHub Actions usa su propio parser. La clave `on` se lee como texto en ambos casos.
- No se ejecutó `actionlint`.
