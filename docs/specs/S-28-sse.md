# Spec S-28: SSE por servidor

- Estado: borrador
- Fase: F6
- Módulo: M6
- Origen en Node (solo lectura): `src/api/sse.js` (`SseHub`, `getSseHub`), `src/api/v1/player.routes.js` (`GET /guilds/:guildId/events`), `src/music/manager.js` (`publishSync`), `src/music/events.js` (llamadas a `publishSync`), `src/api/v1/ops/player.js` (`control`), `src/voice/event-log.js` (publica `voice.log`), `openapi.yaml` (solo documenta `player.sync`).

## Objetivo

Entregar a los suscriptores de un servidor el estado del reproductor al conectarse, y cada cambio después, por Server-Sent Events. Sustituye el sondeo de 2 segundos del dashboard.

Hoy el hub vive en el proceso de la API. En Node, solo salían en vivo los eventos del shard de la API. Con un solo shard, todos salen en vivo.

No incluye las rutas (S-27) ni el registro de voz (S-12), que solo publican en el hub.

## Decisiones

- Un solo shard: todos los eventos del servidor salen en vivo, sin pasar por otro proceso.
- Cada suscriptor tiene una cola de 64 eventos. Si la cola se llena, se cierra ese stream y el cliente reconecta. Node escribe sin límite y guarda todo en memoria.
- `voice.log` se documenta en `openapi.yaml` en un cambio aparte.

## Archivos

- Crear: `rust/crates/bot/src/api/sse.rs`: hub por servidor, suscripción, latido y cierre.
- Modificar: `rust/crates/bot/src/api/rutas/reproductor.rs`: la ruta `GET /guilds/:guildId/events` (S-27).
- Crear: `rust/crates/bot/tests/sse.rs`.
- No tocar: `src/api/sse.js`, `src/api/v1/player.routes.js`, `openapi.yaml`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada: `GET /api/v1/guilds/{guildId}/events` con Bearer (S-26). No lleva `X-User-Discord-Id`, porque es un GET.
- Cabeceras de respuesta: `Content-Type: text/event-stream; charset=utf-8`, `Cache-Control: no-cache, no-transform`, `Connection: keep-alive` y `X-Accel-Buffering: no`.
- Primera línea del stream: `: connected`.
- Evento `player.sync`: `data` es el JSON de `player.state` (S-27). Llega al conectarse y después de cada cambio.
- Evento `voice.log`: `data` es una entrada del registro de voz (S-12). No está en `openapi.yaml`.
- Latido: la línea `: heartbeat` cada 25 segundos.
- Error antes del stream: si el estado inicial falla (por ejemplo, `guild_not_found` 404), responde un problema RFC 7807 y no abre el stream.

## Comportamiento

1. Conexión: valida el snowflake y pide el estado inicial. Si falla, responde el problema. Si sale bien, envía las cabeceras, `: connected`, un `player.sync` con el estado, y registra la suscripción.
2. `publicar_sincronizacion(servidor)`: envía `player.sync` solo si hay suscriptores. Si no los hay, no calcula el estado. Se llama en los mismos puntos que Node: canción nueva, fin de la cola, desconexión del canal, cola borrada (`events.js`), cambios de la cola (`changed`, S-17) y controles de la API (S-27).
3. `voice.log`: se envía a los suscriptores del servidor cuando S-12 añade una entrada. Sin suscriptores, no se envía.
4. Latido: cada 25 segundos, a todos los suscriptores. El temporizador no impide el apagado.
5. Desconexión: al cerrarse el socket se quita la suscripción. Un servidor sin suscriptores sale del mapa.
6. Suscriptor lento o con error: si su cola de 64 eventos se llena, o si falla una escritura, se cierra su stream y se quita de la suscripción. Los demás no se ven afectados. Al reconectar, el cliente recibe el estado completo (regla 1).
7. Orden: cada suscriptor recibe los eventos en el orden en que se publicaron.
8. Apagado (S-26): todos los streams terminan y el latido se detiene.

## Tests

- Suscripción: cabeceras exactas, `: connected` y `player.sync` inicial.
- Estado inicial con error: problema 404, sin stream.
- Sin suscriptores, `publicar_sincronizacion` no calcula el estado (contador de llamadas).
- Dos suscriptores del mismo servidor reciben el evento. Un tercero de otro servidor no lo recibe.
- Latido cada 25 segundos (reloj simulado).
- Socket cerrado: la suscripción se quita, y el servidor vacío sale del mapa.
- Cola llena: se cierra el stream de ese suscriptor, y otro del mismo servidor sigue recibiendo eventos.
- Apagado: todos los streams terminan.
- Paridad: el formato `event:` y `data:` es idéntico al de Node (respuestas grabadas).
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- El dashboard recibe `player.sync` al abrir la página y en cada cambio, sin sondeo.
- Tras un corte de red, el cliente se reconecta y recibe el estado completo.
- Diez cambios seguidos llegan en orden.
- Al apagar el bot, los streams se cierran en menos de 10 segundos (S-21).

## Preguntas abiertas

Ninguna. Las decisiones de F6 están en `## Decisiones`.
