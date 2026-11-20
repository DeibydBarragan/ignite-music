# Spec S-04: Almacén atómico y caché TTL

- Estado: implementada
- Fase: F1
- Módulo: M1
- Origen en Node (solo lectura): `src/core/store.js` (`JsonStore`: `initialize`, `get`, `set`, `delete`, `keys`, `flush`, `drain`), `src/core/cache.js` (`TTLCache`: `get`, `has`, `set`, `delete`, `clear`, `sweep`, `wrap`, y el barrido compartido), `test/core.test.js` (grupos TTLCache y JsonStore).

## Objetivo

Guardar un JSON por clave en disco, de forma atómica, con lectura desde memoria. Dar una caché con caducidad.

Usos: ajustes por servidor (S-03) y estado de sesiones (S-19).

No se porta aquí `Semaphore` de `src/core/cache.js`. Va en S-13, con el límite de procesos de yt-dlp.

## Decisiones

- Si la escritura falla, el valor nuevo queda en memoria, como en Node. El error llega a quien llamó (Comportamiento 3).
- Si la escritura falla, Rust borra el `.tmp` que quedó a medias. Node lo deja.

## Archivos

- Crear: `rust/crates/nucleo/src/almacen.rs` (`AlmacenJson`)
- Crear: `rust/crates/nucleo/src/cache.rs` (`CacheTtl` y barrido)
- Crear: `rust/crates/nucleo/tests/almacen.rs`
- Crear: `rust/crates/nucleo/tests/cache.rs`
- Crear: `rust/crates/nucleo/tests/data/almacen/` (archivos escritos por Node)
- Crear: `scripts/almacen-paridad-ref.js` (debe pasar `npm run lint`)
- No tocar: `src/core/store.js`, `src/core/cache.js`, `test/core.test.js`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada de `AlmacenJson`: carpeta, clave y valor JSON. La clave debe cumplir `^[A-Za-z0-9_-]{1,64}$`.
- Salida de `get`: el valor, o nada si no existe. No toca el disco.
- Salida de `set` y `delete`: resultado de la escritura. Error si falla.
- Entrada de `CacheTtl`: TTL en milisegundos (opcional) y tamaño máximo (por defecto 1000, como en Node). `set` admite un TTL propio para esa entrada; si no, usa el del caché.
- Errores: `ClaveInvalida` (la clave no cumple el patrón, antes de tocar el disco; mensaje de Node: `Invalid store key: <clave>`) y `Escritura` (con la causa de E/S).

## Comportamiento

1. `initialize`: crea la carpeta si falta. Carga en memoria cada `<clave>.json` válido. Un archivo que no se lee o no es JSON se registra con aviso y se omite. No se borra. Si no se puede listar la carpeta, se registra con aviso y se sigue con la caché vacía.
2. `get` solo lee la memoria. Con una clave inválida da error.
3. `set`: cambia la memoria y después espera la escritura. Si la escritura falla, el error llega a quien llamó, igual que en Node (ver Decisiones).
4. Escritura atómica: escribe en `<archivo>.<pid>.<8 hex>.tmp`, hace `sync_all` y renombra sobre `<clave>.json`. Si falla antes del renombrado, el archivo anterior queda intacto y el `.tmp` se borra.
5. Las escrituras de la misma clave van en serie. Las de claves distintas pueden ir en paralelo.
6. `delete` o un valor nulo borran el archivo y dejan la clave vacía en memoria.
7. Formato: JSON compacto, sin sangría, con el mismo orden de claves que Node. En Rust, `serde_json` con la función `preserve_order`.
8. `keys` devuelve solo las claves con valor no nulo.
9. `drain` espera las escrituras pendientes. Ignora sus errores, como Node.
10. TTL: `get` no devuelve la entrada si ya caducó, y la borra. Cada entrada dura `ttl` con ±10 %: `ttl × (0,9 + 0,2 × aleatorio)`.
11. Sin `ttl`, la entrada no caduca. En Node pasa porque el cálculo da NaN. Se conserva.
12. Barrido: cada 60 s se quitan las entradas caducadas de todas las cachés vivas. Hay una sola tarea para todas. No retiene cachés que ya no se usan.
13. Tamaño máximo: al pasar el límite se borra la entrada más antigua por inserción. `get` no cambia el orden.
14. `get_or_load(clave, cargador)` (en Node, `wrap`): si hay un valor vigente, lo devuelve. Si ya hay una carga en curso para esa clave, espera a esa. Si la carga falla, no cachea nada, y la siguiente llamada reintenta.
15. `clear` borra las entradas y las cargas en curso.

## Tests

- Unitarios, equivalentes a `test/core.test.js`: caducidad y carga única con llamadas concurrentes; barrido sin lectura; tamaño máximo; escritura atómica, caché y borrado.
- Propios de Rust:
  - Fallo en el renombrado: el archivo anterior se conserva y no queda `.tmp`.
  - Archivo corrupto: se omite en `initialize` y no se borra.
  - Clave inválida: error antes de tocar el disco.
  - Caché sin `ttl`: la entrada no caduca.
- Paridad con Node: `scripts/almacen-paridad-ref.js` escribe archivos con el `JsonStore` de Node en `tests/data/almacen/`. El test de Rust los lee y compara los valores. En sentido contrario, el test de Rust escribe en una carpeta temporal, y `scripts/almacen-paridad-ref.js --leer <carpeta>` los lee con el `JsonStore` de Node y compara.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Un ajuste de servidor escrito por Node se lee en Rust sin cambios.
- Un ajuste escrito por Rust lo lee el `JsonStore` de Node sin error.
- Tras una escritura correcta no queda ningún archivo `.tmp`.
- `cargo test` pasa. `npm test` pasa sin cambios.

## Preguntas abiertas

- `serde_json` con `preserve_order`: confirmar en el test de paridad que el orden de claves coincide con Node.
