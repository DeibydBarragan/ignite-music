# Spec S-02: Logs y errores

- Estado: implementada
- Fase: F1
- Módulo: M1
- Origen en Node (solo lectura): `src/core/logger.js` (`createLogger`, `normalize`, `setDebug`), `src/api/contracts.js` (`ApiError`, `problem`, `sendProblem`, `sendError`, `sendOpResult`, `STATUS_BY_CODE`, `ERROR_TITLES`), `src/api/server.js` (manejador final de errores), `test/contracts.test.js`.

## Objetivo

Definir cómo se escriben los logs y cómo se representan los errores de la API, en formato RFC 7807. S-26 usa estos errores en las respuestas HTTP.

No incluye los logs de eventos de Discord (S-21) ni las rutas HTTP (S-26).

## Decisiones

- Los errores 500 no `ApiError` llevan el mensaje real del error, como en Node: `error.message`, o `Internal server error` si no hay mensaje. El mensaje real también va al log.
- El campo base `shard` de pino desaparece en Rust: hay un solo proceso.
- Rust no captura backtrace, salvo que `RUST_BACKTRACE` esté activo.
- El campo `error` del documento de error se mantiene hasta que el BFF de Next.js deje de leerlo.

## Archivos

- Crear: `rust/crates/nucleo/src/registro.rs` (inicialización de `tracing`)
- Crear: `rust/crates/nucleo/src/error_api.rs` (`ApiError`, documento de problema, estados)
- Crear: `rust/crates/nucleo/tests/error_api.rs`
- Crear: `rust/crates/nucleo/tests/data/problem-casos.json` (generado con Node)
- Crear: `scripts/problem-paridad-ref.js` (debe pasar `npm run lint`)
- Modificar: `rust/crates/nucleo/src/lib.rs` (exporta los módulos)
- No tocar: `src/api/contracts.js`, `src/core/logger.js`, `test/contracts.test.js`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada de un log: nivel, ámbito (`scope`), mensaje y, si hay, un error con su causa.
- Salida de un log: una línea JSON por evento en producción, con `level`, `time` (ISO 8601), `msg` y `scope`. Texto con color en una terminal.
- Entrada de un error: `ApiError { status, code, detail, title?, type? }`.
- Salida de un error: documento RFC 7807 con `Content-Type: application/problem+json`. Campos: `type`, `title`, `status`, `detail`, `code`, `timestamp` (ISO 8601) y `error`, que repite `detail` por compatibilidad.
- Errores: cualquier error que no sea `ApiError` responde 500 con código `INTERNAL_ERROR` y el mensaje del error (ver Decisiones).

## Comportamiento

1. Cada módulo crea su logger con un ámbito, como `createLogger('scope')` en Node. En Rust es el campo `scope` de `tracing`.
2. Producción es `NODE_ENV=production`. En producción, o cuando no hay color (sin terminal o con `NO_COLOR`), el log sale en JSON por la salida estándar. Si no, sale en texto con color.
3. `DEBUG=true` muestra el nivel debug. Cualquier otro valor lo oculta.
4. Niveles: debug, info, ok, warn y error. El nivel `ok` (35 en Node, entre info y warn) se emite como INFO con el campo `ok = true`.
5. Un error se registra con su mensaje y su stack. Node lo hace con el campo `err` de pino. Rust no captura backtrace salvo con `RUST_BACKTRACE` (ver Decisiones).
6. El código del bot no usa `println!` ni `print!`. Los mensajes van por las macros de log. Solo S-01 escribe en la salida de arranque. Un lint de clippy lo comprueba.
7. `ApiError`: `title` es el que se pasa, o `ERROR_TITLES[status]`, o `Error`. `type` es `https://ignite.bot/errors/` más el código en minúsculas con `_` cambiado por `-`. Por ejemplo, `ALREADY_PLAYING` da `https://ignite.bot/errors/already-playing`.
8. `ERROR_TITLES`: 400 Invalid Request Parameter; 401 Unauthorized; 403 Forbidden; 404 Not Found; 409 Conflict; 413 Payload Too Large; 422 Unprocessable Entity; 429 Too Many Requests; 500 Internal Server Error; 502 Bad Gateway; 503 Service Unavailable.
9. Las opciones extra del documento (campos distintos de `title` y `type`) se añaden al JSON.
10. Un error que no es `ApiError`: 500, `INTERNAL_ERROR`, y `detail` con el mensaje del error (`error.message`; si no hay mensaje, `Internal server error`). El mensaje real también va al log (ver Decisiones).
11. `STATUS_BY_CODE` se porta como tabla: `guild_not_found` 404, `shard_timeout` 503, `not_ready` 503, `unknown_op` 500, `op_failed` 500. No se porta el proxy de shards de Node, que transportaba estos errores.

## Tests

- Unitarios: campos del documento; `type` y `title` derivados; título y tipo propios; tabla `STATUS_BY_CODE`; nivel `ok` como INFO con `ok = true`; `DEBUG` oculta el nivel debug; un `println!` fuera de S-01 falla en clippy.
- Paridad con Node: `scripts/problem-paridad-ref.js` escribe en `problem-casos.json` el documento de Node para una lista de `ApiError` y de errores genéricos. El test de Rust compara `status`, `type`, `title`, `detail`, `code` y `error`. `timestamp` se compara solo por formato. Los casos 500 llevan el mensaje del error, igual que en Node (ver Decisiones).
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- `ApiError(404, GUILD_NOT_FOUND, "x")` da `type` `https://ignite.bot/errors/guild-not-found` y `title` `Not Found`.
- Todos los casos de `problem-casos.json` coinciden con Node, incluidos `detail` y `error` en los 500.
- Ningún `println!` fuera de S-01. Clippy lo marca como error.
- `cargo test` y `npm test` pasan.

## Preguntas abiertas

- Nivel `ok`. Si un colector de logs filtra por `level: ok`, el cambio a INFO lo rompe. Revisar al implementar.
