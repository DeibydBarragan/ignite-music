# Spec S-13: yt-dlp (proceso externo, semáforo y errores)

- Estado: borrador
- Fase: F4
- Módulo: M3
- Origen en Node (solo lectura): `src/music/ytdlp.js` (`resolveBinary`, `proxyFor`, `baseArgs`, `cleanError`, `runJson`, `version`), `src/core/cache.js` (`Semaphore`), `src/music/errors.js` (`errorKey`, `describeError`, `hostHint`), `test/ytdlp.test.js`.

## Objetivo

Ejecutar yt-dlp como proceso externo para cada búsqueda, extracción y lista. Limitar cuántos procesos corren a la vez. Convertir el error de yt-dlp en un motivo corto y en una clave de traducción.

No incluye las fuentes (S-14), el relay ni el token PO (S-15), ni la reproducción (S-17).

## Archivos

- Crear: `rust/crates/medios/src/ytdlp.rs` (binario, argumentos, proceso y JSON)
- Crear: `rust/crates/medios/src/limitador.rs` (semáforo con dos carriles)
- Crear: `rust/crates/medios/src/errores.rs` (motivo corto y clave de traducción)
- Crear: `rust/crates/medios/tests/ytdlp.rs` (binario falso compilado en el propio test)
- Crear: `rust/crates/medios/tests/limitador.rs`
- Crear: `rust/crates/medios/tests/errores.rs`
- Crear: `rust/crates/medios/tests/data/errores-casos.json` (generado con Node)
- Crear: `scripts/errores-paridad-ref.js` (debe pasar `npm run lint`)
- No tocar: `src/music/ytdlp.js`, `src/core/cache.js`, `src/music/errors.js`, `test/ytdlp.test.js`.

El crate `medios` no estaba en S-31. Ver Preguntas abiertas.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada de `ejecutar_json(destino, argumentos, opciones)`:
  - `destino`: URL, o consulta como `ytsearch1:<texto>`.
  - `argumentos`: argumentos extra.
  - `prioridad`: `alta` o `baja`.
  - `tiempo_limite`: por defecto `YTDLP_TIMEOUT`, 30 s.
  - `tiempo_espera_cola`: tiempo máximo para conseguir un hueco.
  - `proxy` y cancelación, opcionales.
- Salida: el JSON que imprime yt-dlp con `--dump-single-json`.
- Errores: `YtDlpError { mensaje, stderr }`. El mensaje tiene como máximo 300 caracteres.
  - Tiempo agotado: `yt-dlp timed out after <N>s`.
  - Binario no encontrado: se añade la pista `yt-dlp binary not found, run "npm install" or set YTDLP_PATH`. En Rust la pista habla de S-32.
  - JSON inválido: `yt-dlp returned invalid JSON`.
- `version()`: `{ version }` o `{ error }`.

## Comportamiento

1. Binario: `YTDLP_PATH` si está definido. Si no, `bin/yt-dlp-dist/yt-dlp` (`.exe` en Windows) si existe. Si no, `yt-dlp` del PATH.
2. Argumentos base, en este orden: `--ignore-config`, `--no-warnings`, `--no-progress`, `--socket-timeout 30`, `--retries 10`, `--fragment-retries 10`, `--retry-sleep fragment:1`, `--buffer-size 16M`, `--force-ipv4` y, si `YTDLP_JS_RUNTIME` tiene valor, `--js-runtimes <valor>`.
3. Cookies: con `COOKIES_FILE`, `--cookies <ruta>`. Si no, y hay `COOKIES_FROM_BROWSER`, `--cookies-from-browser <valor>`.
4. Token PO: si hay token (S-15), `--extractor-args youtube:po_token=web.gvs+<token>;player_client=web,default`. Si hay `YTDLP_EXTRACTOR_ARGS`, se añade otro `--extractor-args` con ese valor.
5. Proxy: con un proxy, `--proxy <url>`.
6. Proxy por servidor: con un solo proxy, o sin clave de servidor, se usa el primero. Con varios, se elige con el SHA-1 de la clave: los primeros 4 bytes como entero sin signo, módulo el número de proxies. Así cada servidor usa siempre el mismo proxy.
7. Llamada: `--dump-single-json -- <destino>`. El destino va después de `--`.
8. Limitador: antes de lanzar el proceso, se pide un hueco. Si no hay hueco en `tiempo_espera_cola`, error. Si se cancela, error. El hueco se libera al terminar, también con error.
9. Proceso: stdout y stderr se leen por separado. Código de salida distinto de 0: error con `cleanError(stderr)`. JSON que no se puede leer: `invalid JSON`. Los avisos en stderr no afectan si el código es 0.
10. Tiempo límite: al agotarse, el proceso se mata (SIGKILL) y el error dice cuánto esperó.
11. `cleanError`: toma la última línea de stderr que empieza por `ERROR:`. Si no hay, la última línea de stderr. Si no hay nada, `yt-dlp failed`. Quita `ERROR: ` del inicio y el prefijo `[extractor] id: `. Corta a 300 caracteres.
12. Limitador: capacidad `YTDLP_CONCURRENCY` (4 por defecto, entre 1 y 32). Tiene dos carriles: alta (acciones del usuario) y baja (precarga). Si hay esperas en los dos, se turnan: tras una de alta, le toca a una de baja. Cada espera tiene tiempo límite y puede cancelarse. Si hay un hueco libre, la tarea empieza en el mismo turno, como en Node.
13. Errores: `errorKey` recorre estas reglas en orden y devuelve la clave de la primera que coincida con el código del error o con su texto en minúsculas. Si ninguna coincide: `errors.unknown`.

    | Condición | Clave |
    | :--- | :--- |
    | VOICE_FULL | `ui.voice_full` |
    | VOICE_MISSING_PERMS | `errors.voice_no_permission` |
    | VOICE_CONNECT_FAILED o VOICE_RECONNECT_FAILED | `musicplayer.failed_connect_voice` |
    | NON_NSFW | `errors.youtube_age_restricted` |
    | NOT_SUPPORTED_URL o NOT_SUPPORTED_SONG | `ui.unsupported` |
    | EMPTY_PLAYLIST o EMPTY_FILTERED_PLAYLIST | `youtube.no_valid_tracks` |
    | NO_RESULT | `musicplayer.no_results_found` |
    | Detección de bot de YouTube (`isBotCheck`) | `errors.youtube_bot_detection` |
    | Texto de edad restringida | `errors.youtube_age_restricted` |
    | Texto de geobloqueo | `errors.youtube_geo_blocked` |
    | 429, "too many requests" o "rate limit" | `errors.rate_limited` |
    | Video no disponible, privado o eliminado | `errors.youtube_unavailable` |
    | Errores de red (ECONNRESET, ETIMEDOUT, ENOTFOUND, EAI_AGAIN, socket hang up, network, timed out) | `errors.network_error` |
    | DRM (texto o código DRM_PROTECTED) | `errors.drm_protected` |
    | FFMPEG_EXITED, NO_STREAM_URL, CANNOT_GET_STREAM_URL, o texto con ffmpeg, 403 o forbidden | `errors.stream_failed` |

14. Búsqueda de texto sin resultados: si YouTube dio un error en los últimos 60 s, se informa ese error y no "sin resultados". Así lo hace `describeError` en Node.
15. Aviso al dueño del bot: para tres claves (`errors.youtube_bot_detection`, `errors.youtube_age_restricted`, `errors.rate_limited`), se escribe en la consola la instrucción de configuración, como mucho una vez cada 10 minutos por clave.

## Tests

- Unitarios de `test/ytdlp.test.js`:
  - Los argumentos base incluyen el runtime de JavaScript, la autenticación y el proxy.
  - `proxyFor` es fijo por servidor y reparte servidores entre proxies.
  - `cleanError` conserva la parte útil de la última línea `ERROR:`.
  - `runJson` lee stdout aunque stderr tenga avisos.
  - `runJson` rechaza con un mensaje limpio cuando falla.
  - `runJson` mata el proceso al agotar el tiempo.
  - `version()` informa la versión.
- Propios de Rust:
  - Limitador: el límite se respeta; las altas van antes que las bajas; se turnan cuando ambos carriles esperan; una espera con tiempo límite falla; una cancelación falla.
  - Errores: cada fila de la tabla con un ejemplo.
- Binario falso: el test compila en Rust un ejecutable pequeño que imprime JSON o sale con error. Así corre también en Windows. Los tests de Node que usan un script de shell se saltan en Windows.
- Paridad con Node: `scripts/errores-paridad-ref.js` evalúa `errorKey` de Node con una lista de códigos y textos (incluidos los de `test/`) y guarda la clave de cada uno en `errores-casos.json`. El test de Rust compara.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Con `YTDLP_CONCURRENCY=2`, nunca hay más de 2 procesos vivos (medido con el binario falso).
- Cada caso de `errores-casos.json` da la misma clave que Node.
- Un proceso que supera el tiempo límite muere, y el error lo dice.
- `cargo test` pasa sin yt-dlp instalado.
- Ningún archivo pasa de 250 líneas.

## Decisiones

- Runtime de JavaScript: `--js-runtimes` toma su valor de la variable `YTDLP_JS_RUNTIME` de `.env`, en lugar de la ruta de Node. Cuando se retire Node (F8), basta con cambiar esa variable. Si falta, S-01 avisa al arrancar (preflight, punto k) y yt-dlp corre sin esa opción.
- `--no-check-certificates` se quita. Node desactiva así la verificación TLS de yt-dlp. Solo se vuelve a poner si F7 muestra fallos de certificado detrás de un proxy.
- Crate `medios` (M3), con yt-dlp y las fuentes (S-14). Se añade al workspace en S-31.

## Preguntas abiertas

- Ninguna en esta spec.
