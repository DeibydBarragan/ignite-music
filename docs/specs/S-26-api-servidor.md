# Spec S-26: Servidor de la API y seguridad

- Estado: borrador
- Fase: F6
- Módulo: M6
- Origen en Node (solo lectura): `src/api/server.js` (`startApiServer`), `src/api/middleware/auth.js` (`requireAuth`, `requireUser`, `extractBearer`, `safeEqual`), `src/api/v1/router.js` (`createRouter`), `src/api/contracts.js` (`sendError`; el formato de errores está en S-02), `src/api/shard-proxy.js` (se retira). Pruebas: `test/api.test.js`.

## Objetivo

Levantar el servidor HTTP de la API dentro del proceso del bot, con axum. Aplica las mismas capas de seguridad que Node, en el mismo orden, y no usa proxy de shards.

No incluye las rutas (S-27), el SSE (S-28), el túnel y el anuncio (S-29) ni los datos (S-30).

## Decisiones

- La API vive en el proceso del bot (decisión 5 del plan). Las operaciones se ejecutan en el mismo proceso, sin `broadcastEval`.
- La API escucha solo en `127.0.0.1`. Node escucha en `0.0.0.0`. El túnel (S-29) llega desde el mismo equipo, así que no necesita la red local. Antes de F7 hay que confirmar que ningún cliente llama a la API por la IP de la red local.
- JSON mal formado: 400 `INVALID_JSON`. Cuerpo de más de 100 kB: 413 `PAYLOAD_TOO_LARGE`. Node responde 500 en los dos casos.
- Error no controlado: 500 `INTERNAL_ERROR` con el detalle "Error interno del servidor.". El mensaje real va al log (S-02). Node envía su mensaje interno al cliente.
- Límite de peticiones: 429 `RATE_LIMITED` como problema RFC 7807, con el detalle "Demasiadas peticiones. Inténtalo más tarde.". Node responde texto `text/html`.
- Ruta inexistente: 404 `NOT_FOUND` como problema RFC 7807, con el detalle "Ruta no encontrada.". Node responde HTML.

## Archivos

- Crear: `rust/crates/bot/src/api/mod.rs`: arranque del servidor y orden de las capas.
- Crear: `rust/crates/bot/src/api/seguridad.rs`: Bearer, `X-User-Discord-Id` y comparación en tiempo constante.
- Crear: `rust/crates/bot/src/api/capas.rs`: cabeceras, CORS, límite de cuerpo, identificador de petición y límite de peticiones.
- Modificar: `rust/crates/bot/Cargo.toml`: `axum`, `tower-http` y `tokio`, con versiones fijadas al añadirlas.
- Crear: `rust/crates/bot/tests/api_seguridad.rs` y `rust/crates/bot/tests/api_capas.rs`.
- No tocar: `src/api/`, `test/api.test.js`, `openapi.yaml`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Configuración (S-01): `API_ENABLED` (verdadero por defecto), `API_PORT` (3001), `API_SECRET` (32 caracteres o más), `API_ALLOWED_ORIGINS` y `API_RATE_LIMIT_PER_MINUTE` (120).
- Escucha: `127.0.0.1` en el puerto `API_PORT`. Ver Decisiones.
- Salida: JSON (`application/json`) o problema RFC 7807 (`application/problem+json`, con la forma de S-02).
- Errores de esta capa: 503 `AUTH_NOT_CONFIGURED`, 401 `UNAUTHORIZED`, 400 `INVALID_USER`, 400 `INVALID_JSON`, 413 `PAYLOAD_TOO_LARGE`, 429 `RATE_LIMITED`, 404 `NOT_FOUND` y 500 `INTERNAL_ERROR`.

## Comportamiento

1. Límite de peticiones: ventana fija de 60 segundos por IP, con el límite de `API_RATE_LIMIT_PER_MINUTE`. Va antes de la autenticación. Envía `X-RateLimit-Limit`, `X-RateLimit-Remaining` y `X-RateLimit-Reset`, y `Retry-After` en la respuesta 429. Son las cabeceras que envía Node hoy: `legacyHeaders` está encendido y `standardHeaders` apagado.
2. CORS: permite los orígenes de `API_ALLOWED_ORIGINS`. Con `*`, permite todos. Una petición sin cabecera `Origin` (servidor a servidor, el BFF) pasa sin filtro. Un origen no listado no recibe cabeceras CORS, pero la petición sí se procesa, porque CORS no es autenticación. Las peticiones `OPTIONS` se contestan aquí, antes de autenticar.
3. Cabeceras de seguridad, iguales a las de `helmet()` con sus valores por defecto. `X-Powered-By` queda desactivado. La lista exacta se copia de la respuesta de Node en la prueba de paridad.
4. Cuerpo JSON de hasta 100 kB. Un JSON mal formado responde 400 `INVALID_JSON`. Un cuerpo mayor responde 413 `PAYLOAD_TOO_LARGE`.
5. Identificador de petición: 8 caracteres, el inicio de un UUID v4. Sirve para correlacionar logs.
6. Salud: `GET /api/health` y `GET /api/v1/health`. Sin autenticación. Responden `{"status": "ok", "uptime": <segundos enteros>}`.
7. Resto de rutas, bajo `/api/v1` y `/api` (el mismo router con dos prefijos, S-27):
   1. Bearer. Si `API_SECRET` tiene menos de 32 caracteres, responde 503 `AUTH_NOT_CONFIGURED` y registra un error. Si el token falta, usa otro esquema o no coincide, responde 401 `UNAUTHORIZED`. El esquema se lee sin distinguir mayúsculas. La comparación es en tiempo constante: primero se comparan las longitudes y después los bytes.
   2. Mutaciones (POST, PUT, PATCH y DELETE): `X-User-Discord-Id` debe ser un snowflake de 17 a 20 dígitos. Si no, responde 400 `INVALID_USER`. El valor queda como usuario de la petición.
   3. Validación de la entrada y operación (S-27).
8. Error no controlado: problema RFC 7807 con 500 `INTERNAL_ERROR` y el detalle "Error interno del servidor.". El mensaje real se registra en el log con el identificador de la petición.
9. Ruta inexistente: problema RFC 7807 con 404 `NOT_FOUND` y el detalle "Ruta no encontrada.".
10. Apagado (S-21): el servidor deja de aceptar conexiones y cierra las abiertas.

## Tests

- Bearer: esquema en minúsculas, esquema ajeno, valor vacío, espacios sobrantes, longitud distinta y valor igual.
- `API_SECRET` de 31 caracteres: toda ruta responde 503, y la salud responde 200.
- Mutaciones sin cabecera o con 16 dígitos: 400. Con 17 o 20 dígitos: la capa las deja pasar.
- CORS: origen listado, origen no listado, `*`, sin `Origin` y preflight `OPTIONS`.
- Errores: JSON mal formado (400 `INVALID_JSON`), cuerpo de 100 kB más un byte (413), límite agotado (429 `RATE_LIMITED` con `Retry-After`), ruta inexistente (404) y error interno (500 sin el mensaje interno).
- Paridad: los mismos casos de `test/api.test.js`, con la misma forma de respuesta. Quedan fuera los errores cambiados en Decisiones.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- El proceso escucha en `127.0.0.1:API_PORT`, y no en la red local. Al apagar el bot, el puerto queda libre.
- Sin Bearer válido, toda ruta responde 401, salvo la salud.
- Una mutación sin `X-User-Discord-Id` válido responde 400.
- `cargo test` pasa sin Discord.
- Ningún archivo pasa de 250 líneas.

## Preguntas abiertas

Ninguna. Las decisiones de F6 están en `## Decisiones`.
