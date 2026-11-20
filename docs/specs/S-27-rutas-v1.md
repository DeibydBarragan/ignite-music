# Spec S-27: Rutas v1

- Estado: implementado
- Fase: F6
- Módulo: M6
- Origen en Node (solo lectura): `src/api/v1/router.js`, `src/api/v1/player.routes.js`, `queue.routes.js`, `discovery.routes.js`, `voice.routes.js`, `dev.routes.js`; operaciones en `src/api/v1/ops/` (`player.js`, `queue.js`, `discovery.js`, `voice.js`); validación en `src/api/middleware/validate.js`; formas en `src/api/contracts.js`; contrato en `openapi.yaml`. `player.js` y `discovery.js` tienen cambios sin commit del usuario (modo dev). Esta spec sigue el archivo de trabajo.

## Objetivo

Portar las 30 rutas v1 con las mismas entradas, validaciones y respuestas que usa el dashboard hoy. Manda el código, porque es lo que consume el frontend. `openapi.yaml` se corrige en un cambio aparte.

No incluye la autenticación ni los límites (S-26), el SSE (S-28), los ajustes de voz en Supabase (S-30) ni el registro de voz (S-12).

## Decisiones

- El dashboard no cambia. El criterio de F6 del plan exige que funcione sin cambios contra Rust.
- Contrato: Rust sirve lo que consume el frontend, es decir, el código. `openapi.yaml` se corrige en un cambio aparte. Las 14 rutas que no están en `openapi.yaml` se documentan en otro cambio aparte.
- Códigos de error: se mantienen tal cual, por paridad. Unificarlos es un cambio aparte.
- No se producen los códigos de la capa de shards (`shard_timeout`, `not_ready`, `unknown_op`), porque no hay shards. Un error sin código responde `op_failed`.
- Los textos de las operaciones se quedan en español, por paridad. Pasarlos a i18n es un cambio aparte.
- `GET /voice/logs` tiene tope de `limit` de 100. Node no tiene tope.
- Esta spec describe el archivo de trabajo de `player.js` y `discovery.js`, con los cambios de modo dev sin commit. Si el usuario cambia esa regla, se corrige aquí.

## Archivos

- Crear: `rust/crates/bot/src/api/rutas/mod.rs`: registro de rutas y prefijos.
- Crear: `rust/crates/bot/src/api/rutas/reproductor.rs`, `cola.rs`, `descubrimiento.rs`, `voz.rs` y `dev.rs`.
- Crear: `rust/crates/bot/src/api/validacion.rs`: esquemas, snowflakes e `inspeccionar_destino`.
- Crear: `rust/crates/bot/src/api/formas.rs`: serialización de pistas y listas, y problemas.
- Crear: `rust/crates/bot/tests/rutas_v1.rs` y `rust/crates/bot/tests/fixtures/rutas-node.json` (respuestas grabadas de Node).
- No tocar: `src/api/`, `openapi.yaml`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

Todas las rutas empiezan por `/api/v1`, y por compatibilidad también por `/api`. Las rutas por servidor siguen el patrón `/guilds/:guildId`.

| Método | Ruta | Operación | En openapi |
| :--- | :--- | :--- | :--- |
| GET | `/guilds/:guildId/player` | `player.state` | sí |
| POST | `/guilds/:guildId/play` | `player.play` | sí |
| POST | `/guilds/:guildId/control` | `player.control` | sí |
| GET | `/guilds/:guildId/events` | SSE (S-28) | sí |
| DELETE | `/guilds/:guildId/queue` | `queue.clear` | sí |
| POST | `/guilds/:guildId/queue/undo` | `queue.undo` | sí |
| DELETE | `/guilds/:guildId/queue/:index` | `queue.removeIndex` | sí |
| DELETE | `/guilds/:guildId/queue/items/:trackId` | `queue.removeTrack` | sí |
| POST | `/guilds/:guildId/queue/move` | `queue.move` | sí |
| PATCH | `/guilds/:guildId/queue/order` | `queue.order` | sí |
| POST | `/guilds/:guildId/queue/jump` | `queue.jump` | sí |
| GET | `/guilds/:guildId/top-songs` | `discovery.topSongs` | sí |
| GET | `/guilds/:guildId/voice-status` | `guild.voiceStatus` | no |
| GET | `/guilds/:guildId/channels` | `guild.channels` | no |
| GET | `/guilds/:guildId/recommendations` | `discovery.recommendations` | no |
| GET | `/guilds/:guildId/playlist/resolve` | `playlist.resolve` | no |
| GET | `/guilds/:guildId/voice` | `voice.get` (S-30) | no |
| PUT | `/guilds/:guildId/voice` | `voice.update` (S-30) | no |
| POST | `/guilds/:guildId/voice/reset` | `voice.reset` (S-30) | no |
| GET | `/guilds/:guildId/voice/logs` | `voice.logs` (S-12) | no |
| DELETE | `/guilds/:guildId/voice/logs` | `voice.clearLogs` (S-12) | no |
| POST | `/guilds/:guildId/voice/join` | `voice.join` | no |
| POST | `/guilds/:guildId/voice/leave` | `voice.leave` | no |
| GET | `/health` | salud (S-26) | sí |
| GET | `/guilds` | lista de servidores | sí |
| GET | `/search` | `discovery.search` o `discovery.searchPlaylists` | sí |
| GET | `/search/suggest` | `discovery.suggest` | sí |
| GET | `/search/playlists` | `discovery.searchPlaylists` | no |
| GET | `/user/dev-mode` | modo dev | no |
| POST | `/user/dev-mode` | modo dev | no |

Validación (se rechaza antes de la operación):

- `play`: `query` de 1 a 1000 caracteres. `voiceChannelId` y `userId`: vacíos, nulos o snowflakes.
- `control`: `action` de 1 a 32 caracteres. `value`: texto de hasta 200, número, booleano o nulo.
- `move`, `jump` y `order`: enteros de 1 o más. `trackId`: de 8 a 64 caracteres alfanuméricos, `_` o `-`.
- Consultas: `search` (`q` de hasta 500, `limit` de 1 a 15, `type` `video` o `playlist`), `suggest` (`q`), `top-songs` (`timeframe` `week`, `month` o `all`; `limit` de 1 a 100), `resolve` (`url` obligatoria, de 1 a 1000), `voice-status` (`userId` snowflake opcional) y `voice/logs` (`limit` con tope de 100).
- Un error de validación responde 400 `INVALID_PARAMETER`, o `INVALID_SNOWFLAKE` si falla un parámetro de ruta. El `detail` dice el campo y el motivo.

## Comportamiento

1. Orden: autenticación y `X-User-Discord-Id` (S-26), validación y operación. Una operación responde JSON. Un error de operación responde un problema RFC 7807 con su código. Un error sin código responde `op_failed`.
2. Filtro de destinos (`inspeccionar_destino`): se aplica a `play` (campo `query`) y a `playlist/resolve` (`url`). Responde 400 `BLOCKED_URL` si el destino:
   - usa `file:`, `data:`, `javascript:`, `vbscript:` o `blob:`;
   - usa un esquema que no es http ni https;
   - apunta a un host local o privado: `localhost`, `*.localhost`, `*.local`, `*.internal`, `*.home.arpa`, `metadata.google.internal`, IPv4 de 0, 10, 127, 169.254, 172.16 a 172.31 y 192.168, IPv6 `::1`, `::`, `fc00::/7` y `fe80::/10`, y las IPv4 dentro de IPv6 (`::ffff:`, en decimal o en hexadecimal).
   El texto de búsqueda sin esquema pasa.
3. `play`: si `unsupportedUrlReason` marca el destino, responde 400 `UNSUPPORTED_URL`. Ese código es distinto de `BLOCKED_URL`. El canal se elige así: el del usuario; si el bot está en otro canal, 400 `WRONG_VOICE_CHANNEL` (salvo en modo dev); si no, `voiceChannelId`; si no, en modo dev, el canal del bot, el de la cola o uno disponible; si no hay nada, 400 `NOT_IN_VOICE`. Con `skip`, reanuda después de saltar. Responde `{success, started, position, song, playlist}`.
4. `control`: la acción debe estar en el conjunto de 14: `pause`, `resume`, `toggle`, `skip`, `previous`, `stop`, `shuffle`, `autoplay`, `filter`, `volume`, `seek`, `loop`, `deafen` y `undeafen`. Si no, responde 400 `INVALID_ACTION`. Después de la acción publica `player.sync` (S-28). Responde `{success, message}`.
5. `player.state`: `repeatMode` vale `off`, `track` o `queue`, no `song`. Sin cola: `hasQueue: false`, volumen por defecto y `queue: []`.
6. Cola: cada mutación corre bajo el bloqueo del servidor (S-17). Errores: `NO_QUEUE` 404, `INVALID_INDEX` 400, `TRACK_NOT_FOUND` 404, `INVALID_MOVE` 400 e `INVALID_JUMP` 400. `DELETE /queue` guarda la cola 60 segundos para deshacer. `POST /queue/undo` responde `UNDO_EMPTY` 400 (sin datos), `UNDO_EXPIRED` 400, `NO_ACTIVE_PLAYER` 404, `guild_not_found` 404 o `UNDO_FAILED` 422.
7. Búsqueda: `GET /search` lee `q` (no `query`), con `limit` 6 por defecto, y responde `{query, results}`. Si la búsqueda falla, responde 200 con `results: []`, así que el error no llega al cliente. `GET /search/suggest` responde `{query, suggestions}`. `GET /guilds` responde `{guilds: [{id, name, icon, memberCount}]}`.
8. `top-songs`: `timeframe` por defecto `week`, y `limit` por defecto 100. El resultado tiene como máximo 50 canciones. Responde `{success, guildId, timeframe, total, songs}`. Cada canción incluye `playCount`, `rank` y `lastPlayed`. El campo `total` coincide con la cantidad devuelta.
9. `recommendations`: con `force=true` o `refresh=true` borra la caché de 20 minutos de esa canción. Con resultados relacionados responde `source: "radio"` y `basedOn` describe la pista activa. Si no hay radio, devuelve el ranking con los mismos campos de `top-songs`, incluso si el ranking está vacío. `"empty"` indica un fallo al obtener el ranking.
10. `voice-status`: responde `{bot, user, canControl}`. Si el bot está ensordecido en el servidor, intenta quitárselo con `setDeaf(false)`. Es un efecto secundario de un GET (ver Preguntas).
11. `playlist/resolve`: devuelve `{success, kind, name, source, count, truncated, tracks}` con los IDs y metadatos del extractor. Las pistas resueltas no incluyen campos de cola. Si falla, responde 422 `UNSUPPORTED_URL` o `RESOLVE_FAILED`. En `play`, una URL no soportada responde 400.
12. Voz: `join` elige el canal indicado, o el del usuario, o uno con personas, o cualquiera. Sin canal, responde 400 `no_voice_channel`. Sin permiso Connect, 403 `voice_permission_denied`. Responde `{success, channel, message}`. `leave` para la cola, sale del canal y responde `{success, message}`. `logs` responde `{logs}` y acepta `limit`, con tope de 100. `clearLogs` responde `{ok: true}`. Los ajustes de voz están en S-30.
13. Modo dev: `GET /user/dev-mode` responde `{success, devMode, userId}`. `POST` con `enabled` cambia el modo y responde lo mismo. Sin usuario, `POST` responde 400 `MISSING_USER_ID` (ver Preguntas). La lista se guarda en `DATA_DIR/dev_users.json`. Esta parte está en el archivo de trabajo del usuario, sin commit.

## Tests

- Una prueba por ruta, con éxito y con error, contra `tests/fixtures/rutas-node.json` (forma del cuerpo y código).
- Validación: los límites de cada esquema, enteros enviados como texto (`"3"`), snowflakes vacíos y `voice/logs` con `limit` de 500, que devuelve como máximo 100.
- Filtro de destinos: públicos, `localhost`, `127.0.0.1`, `10.0.0.1`, `169.254.169.254`, `[::1]`, `[::ffff:7f00:1]`, `file:`, `javascript:`, `ftp:` y texto de búsqueda.
- Control: los 14 valores pasan. Otro valor responde `INVALID_ACTION`.
- Cola: `removeIndex`, `removeTrack`, `move`, `order` y `jump`. `clear` y `undo` dentro de 60 segundos y después de 60 segundos (reloj simulado).
- Recomendaciones: `force` borra la caché.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Las 30 rutas responden con la misma forma que Node en la prueba de paridad, salvo lo aprobado en Decisiones.
- El dashboard (`ignite-frontend`) funciona sin cambios: reproducir, controlar, cola, búsqueda, top, ajustes de voz y modo dev.
- Ninguna ruta usa un cuerpo sin validar.
- `cargo test` pasa.

## Preguntas abiertas

1. Modo dev sin usuario. `POST /user/dev-mode` sin usuario responde 400 `MISSING_USER_ID` con `{success, error, detail}`, que no es un problema RFC 7807. Con la cabecera obligatoria de S-26, esta respuesta no debería alcanzarse. Propuesta: confirmarlo en la prueba de paridad y, si se alcanza, responder problema RFC 7807 con el mismo código. Cambia el comportamiento: pide OK.
