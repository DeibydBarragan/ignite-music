# Spec S-29: Túnel de Cloudflare y anuncio de la URL

- Estado: borrador
- Fase: F6
- Módulo: M6
- Origen en Node (solo lectura): `src/api/tunnel.js` (`startTunnel`, `stopTunnel`, `getTunnelUrl`), `src/api/announce.js` (`publicUrl`, `beat`, `startAnnouncer`), `src/setup/launcher.js` (arranque y parada del túnel), `src/core/binaries` (`ensureCloudflared`, ver S-32).

## Objetivo

Exponer la API local con un túnel rápido de Cloudflare, publicar su URL en `bot_status` para que el dashboard la encuentre, y cerrar el túnel al apagar. Solo se activa con `AUTO_TUNNEL`.

No incluye la API (S-26 y S-27) ni la instalación de `cloudflared` (S-32).

## Decisiones

- Sin reinicio de `cloudflared`. Si el proceso muere, la URL se borra y no vuelve sola. Los cortes del túnel se miden en F7 (S-33).
- La URL pública va en `bot_status`, que tiene lectura pública (S-30). La API exige Bearer para sus rutas (S-26).
- Dominio propio con túnel con nombre: después de F8.

## Archivos

- Crear: `rust/crates/bot/src/api/tunel.rs`: proceso `cloudflared`, lectura de la URL y tiempo límite.
- Crear: `rust/crates/bot/src/api/anuncio.rs`: URL pública y latido de 60 segundos.
- Crear: `rust/crates/bot/tests/tunel.rs` y `rust/crates/bot/tests/anuncio.rs`.
- No tocar: `src/api/tunnel.js`, `src/api/announce.js`, `src/setup/launcher.js`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada: `API_ENABLED`, `AUTO_TUNNEL` (falso por defecto), `API_PORT`, `PUBLIC_API_URL` (opcional) y la conexión a Supabase (S-30). Binario `cloudflared`: `bin/cloudflared.exe` (S-32), después las rutas de respaldo y por último el PATH.
- Salida: una URL `https://<nombre>.trycloudflare.com`, o nada si falla.
- Fila en `bot_status`: `{id: "music", api_url, updated_at}`, con upsert por `id`.

## Comportamiento

1. Arranque: solo si `API_ENABLED` y `AUTO_TUNNEL` están activos. Busca `cloudflared`. Lo lanza con `tunnel --protocol http2 --url http://localhost:<API_PORT>`, sin consola visible y con stdout y stderr en tuberías.
2. Lee stdout y stderr hasta la primera coincidencia de `https://[a-zA-Z0-9-]+\.trycloudflare\.com`.
3. Tiempo límite de 25 segundos. Si vence, mata el proceso y devuelve nada. El arranque sigue sin túnel.
4. Binario ausente o salida temprana: devuelve nada y avisa en el log.
5. Si el túnel ya tiene URL, la devuelve sin lanzar otro proceso.
6. Si el proceso muere después de dar URL, borra la URL y avisa. No se reinicia (ver Decisiones).
7. Parada (apagado de S-21): envía SIGTERM. Si el proceso no sale, lo mata. Borra la URL.
8. URL pública: la del túnel si existe. Si no, `PUBLIC_API_URL` sin barras finales. Si no empieza por http o https, queda vacía.
9. Anuncio: sin URL pública o sin Supabase, no hace nada. Con URL, escribe en `bot_status` al arrancar y cada 60 segundos. Un error de escritura queda en el log y no detiene el latido.
10. Orden: el anuncio arranca después del túnel, cuando el bot conecta (S-21).

## Tests

- Extracción de la URL: una línea con URL válida, un dominio ajeno y texto antes y después de la URL.
- Tiempo límite: proceso falso que no imprime URL. Devuelve nada y el proceso termina.
- Binario inexistente: devuelve nada, sin pánico.
- Con `cloudflared` apuntando a `http://localhost:<API_PORT>` y la API en `127.0.0.1` (S-26), el túnel llega a la API.
- Segunda llamada con túnel activo: la misma URL y ningún proceso nuevo.
- Parada: SIGTERM y limpieza de la URL.
- URL pública: con y sin túnel, con `PUBLIC_API_URL` con barra final, y con un esquema no válido.
- Latido: upsert cada 60 segundos (reloj simulado). Un error de Supabase no detiene el latido.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Con `AUTO_TUNNEL=true`, el log muestra la URL en menos de 25 segundos.
- El dashboard, con esa URL en `bot_status`, recibe `player.sync` (S-28) a través del túnel.
- Al apagar el bot no queda ningún proceso `cloudflared`.
- Sin `AUTO_TUNNEL`, `cloudflared` no se lanza.

## Preguntas abiertas

Ninguna. Las decisiones de F6 están en `## Decisiones`.
