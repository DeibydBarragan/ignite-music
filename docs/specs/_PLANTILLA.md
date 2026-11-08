# Spec <ID>: <nombre>

- Estado: borrador | aprobada | en curso | cerrada
- Fase: <F#>
- Módulo: <M#>
- Origen en Node (solo lectura): <ruta> (<funciones>)

## Objetivo

Qué hace la pieza y qué no hace.

## Archivos

- Crear: `<ruta>`
- Modificar: `<ruta>`
- No tocar: `<ruta>`

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada: tipo, unidades y formato.
- Salida: tipo, unidades y formato.
- Errores: cada caso y lo que devuelve.

## Comportamiento

Reglas numeradas. Cada regla se puede probar.

## Tests

- Unitarios: lista.
- Paridad con Node: qué se compara y cómo.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

Lista verificable. Cada punto se comprueba con un comando o una prueba.

## Preguntas abiertas

Lo que aún no se sabe, y cómo se resuelve.
