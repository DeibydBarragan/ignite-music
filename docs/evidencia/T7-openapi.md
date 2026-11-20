# T7: inventario de operaciones de openapi.yaml

## Resultado

El API v1 tiene 30 operaciones (método + ruta) en Rust y en Node. Las 30 están documentadas en `openapi.yaml`. No hubo diferencias entre los tres inventarios.

Rust registra 29 rutas en `rust/crates/bot/src/api/rutas/mod.rs` (función `crear_subrouter`) y una más, `GET /api/v1/health`, en `rutas_publicas` de `rust/crates/bot/src/api/mod.rs`. Node registra 29 rutas en `src/api/v1/*.routes.js` y una más, `GET /api/v1/health`, en `src/api/server.js`. La cifra 30 incluye el health. La documentación original decía 27 claves de ruta. Son 27 rutas con 30 pares método + ruta, porque varias rutas tienen más de un método.

## Tabla de operaciones

| Método | Ruta | Rust | Node | openapi.yaml |
|---|---|---|---|---|
| GET | /api/v1/health | sí (público) | sí (server.js) | sí |
| GET | /api/v1/guilds/{guildId}/player | sí | sí | sí |
| POST | /api/v1/guilds/{guildId}/play | sí | sí | sí |
| POST | /api/v1/guilds/{guildId}/control | sí | sí | sí |
| GET | /api/v1/guilds/{guildId}/events | sí | sí | sí |
| DELETE | /api/v1/guilds/{guildId}/queue | sí | sí | sí |
| POST | /api/v1/guilds/{guildId}/queue/undo | sí | sí | sí |
| DELETE | /api/v1/guilds/{guildId}/queue/{index} | sí | sí | sí |
| DELETE | /api/v1/guilds/{guildId}/queue/items/{trackId} | sí | sí | sí |
| POST | /api/v1/guilds/{guildId}/queue/move | sí | sí | sí |
| PATCH | /api/v1/guilds/{guildId}/queue/order | sí | sí | sí |
| POST | /api/v1/guilds/{guildId}/queue/jump | sí | sí | sí |
| GET | /api/v1/guilds | sí | sí | sí |
| GET | /api/v1/search | sí | sí | sí |
| GET | /api/v1/search/suggest | sí | sí | sí |
| GET | /api/v1/search/playlists | sí | sí | sí |
| GET | /api/v1/guilds/{guildId}/top-songs | sí | sí | sí |
| GET | /api/v1/guilds/{guildId}/voice-status | sí | sí | sí |
| GET | /api/v1/guilds/{guildId}/channels | sí | sí | sí |
| GET | /api/v1/guilds/{guildId}/recommendations | sí | sí | sí |
| GET | /api/v1/guilds/{guildId}/playlist/resolve | sí | sí | sí |
| GET | /api/v1/guilds/{guildId}/voice | sí | sí | sí |
| PUT | /api/v1/guilds/{guildId}/voice | sí | sí | sí |
| POST | /api/v1/guilds/{guildId}/voice/reset | sí | sí | sí |
| GET | /api/v1/guilds/{guildId}/voice/logs | sí | sí | sí |
| DELETE | /api/v1/guilds/{guildId}/voice/logs | sí | sí | sí |
| POST | /api/v1/guilds/{guildId}/voice/join | sí | sí | sí |
| POST | /api/v1/guilds/{guildId}/voice/leave | sí | sí | sí |
| GET | /api/v1/user/dev-mode | sí | sí | sí |
| POST | /api/v1/user/dev-mode | sí | sí | sí |

Node usa `:guildId` y Rust usa `{guildId}`. Es solo la sintaxis del parámetro, no una diferencia de ruta.

## Prueba

`test/openapi.test.js` tiene una lista fija de 30 operaciones, derivada de la tabla anterior. La prueba lee las operaciones de `openapi.yaml` y falla si el conjunto difiere de la lista en cualquier sentido. Se comprobó con una copia temporal de `openapi.yaml` sin la operación `DELETE /voice/logs`: la prueba falló (`not ok 1`, `pass 0`, `fail 1`).

## Comandos

| Comando | Resultado |
|---|---|
| `node --test test/openapi.test.js` | 1 pasa, 0 fallan |
| `npm test` | 234 pruebas: 227 pasan, 0 fallan, 7 omitidas (scripts con shebang, requieren shell POSIX; esta máquina es Windows) |
| `npm run lint` | sin errores (código de salida 0) |
| `npx --yes @redocly/cli@1.34.5 lint openapi.yaml` (comando de `.github/workflows/rust.yml`) | "valid", 0 errores, 41 avisos |

Los 41 avisos de Redocly son de dos tipos: operaciones sin `operationId` y operaciones sin respuesta 4XX. No bloquean la CI. No se corrigieron porque están fuera del alcance de T7.

## Notas sobre el contrato (no cambiadas)

Según la regla de no cambiar operaciones ya documentadas, estos puntos quedan anotados sin editar:

- En `/api/v1/guilds/{guildId}/voice/join` (`channelId` y `userId`) y en el esquema `VoiceChannel` (`id`), el patrón está escrito como `'^\\d{17,20}$'` entre comillas simples. En YAML las comillas simples no escapan la barra, así que el regex real es `\\d` (barra literal seguida de `d`). Ese patrón no acepta IDs de Discord. Debería ser `'^\d{17,20}$'`, como en el resto del archivo.
- Los 41 avisos de Redocly (ver arriba) no se corrigieron.

## Estado de git

`openapi.yaml` ya tenía cambios sin commit antes de esta tarea (445 líneas). `test/openapi.test.js` es un archivo nuevo sin seguimiento. No se hizo commit, stash, reset, checkout ni push. Ningún archivo bajo `rust/` se tocó.
