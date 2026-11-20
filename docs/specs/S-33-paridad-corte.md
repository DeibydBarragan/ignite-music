# Spec S-33: Paridad, medición en paralelo y corte a Rust

- Estado: en curso (herramientas listas, ejecución operativa pendiente)
- Fase: F7
- Módulo: M8
- Origen en Node (solo lectura): `src/music/events.js` (`PLAY_SONG`, `FINISH_SONG`, `ERROR`), `src/voice/stt.js` (`transcribeUtterance`), `src/voice/groq-client.js`, `src/music/errors.js`, `src/core/youtube-guards.js` (`isBotCheck`), `src/music/pot.js`, `src/core/logger.js`, `src/api/announce.js`, `src/setup/launcher.js` (parada del túnel), `start.bat`.

## Objetivo

Cerrar F7 en tres pasos:

1. Comprobar la lista de paridad de F1 a F6 en el servidor de prueba.
2. Medir 24 horas con Node y Rust en paralelo, cada uno con su bot de prueba.
3. Pasar el bot de producción a Rust. Node se para, sin cambios, y queda listo para volver.

No incluye la instalación de binarios (S-32) ni la retirada de Node (F8).

## Decisiones

- Antes de empezar hacen falta: un segundo bot de prueba (lo crea el usuario, el primero ya existe), dos canales de voz distintos en el servidor de prueba, el ID de Discord del operador para la API, la migración de `user_favorites` en el repo y la comprobación de la zona horaria (S-30).
- Cada bot de prueba es una aplicación de Discord distinta. `CLIENT_ID` sale del token (S-01, paso c), y cada bot registra sus comandos en su propia aplicación.
- Los bots de prueba no anuncian URL (`AUTO_TUNNEL=false`, sin `PUBLIC_API_URL`). Así no escriben la fila `music` de `bot_status` (S-29).
- El lanzador de prueba fija variables de entorno, y `.env` no cambia. Las variables del entorno ganan sobre `.env` (S-01).
- `DEBUG=true` en los dos bots, para que ffmpeg y yt-dlp escriban la misma salida. Con la misma carga, el coste de logs es igual.
- Node no cambia en F7. La medición usa sus mensajes de log.
- Un corte sigue el criterio de `FINISH_SONG`: pista de más de 30 segundos, reproducida al menos 5 segundos y parada más de 12 segundos antes del final. Se cuenta una vez por pista, como Node.
- Los logs de Node incluyen los 7 primeros caracteres de la clave de Groq (`keyPrefix`, nivel info en `groq-client.js`). Los logs de prueba se guardan fuera del repositorio y no se comparten.
- Duración: 24 horas seguidas, sin reiniciar ninguno de los bots. S-15 pide 24 horas como mínimo.
- Umbral: Rust no es peor que Node. No hay margen en cortes, 403, bot-check ni pistas que no suenan. El usuario fija el margen de CPU, RAM y latencias antes de F7.
- Escalonado: Node busca en el minuto 0 de cada 10 minutos, y Rust en el minuto 5. Los dos bots comparten IP, y YouTube aplica límites por IP.
- Token en el corte: no se rota el token en el portal de Discord. El corte solo cambia qué proceso lo usa. Rotarlo impide la vuelta atrás.
- pot.js: si Rust sin pot no supera a Node con pot en 403 ni en bot-check, pot.js se retira (F4 a). Si lo supera, se reabre la decisión y se evalúa el Plan B (relay, S-15).
- Carga: las cinco consultas de `carga.ps1` son canciones fijas que elige el usuario antes de F7.
- Resultados: la lista de paridad y sus resultados van en `docs/paridad-f7.md`, creado al empezar F7.

## Archivos

- `..\pruebas-f7\`: fuera del repositorio: datos de cada bot, binarios de Rust de prueba, logs, CSV y resumen. No se commitea (creado por los scripts).
- `rust/scripts/prueba/lanzar-node.bat` (29 líneas): fija variables de prueba y manda salida a `..\pruebas-f7\logs\node.log`.
- `rust/scripts/prueba/lanzar-rust.bat` (45 líneas): fija variables de prueba y manda salida a `..\pruebas-f7\logs\rust.log`.
- `rust/scripts/prueba/medir.ps1` (113 líneas): muestreo de CPU y RAM cada 60 segundos con proceso raíz y descendientes.
- `rust/scripts/prueba/carga.ps1` (101 líneas): llamadas escalonadas a la API de cada bot cada 10 minutos con 5 búsquedas y 1 play.
- `rust/scripts/prueba/resumen.ps1` (147 líneas): genera `resumen.md` comparativo calculando percentiles y tasas por hora.
- `docs/paridad-f7.md` (98 líneas): lista de paridad con resultados de pruebas automáticas y protocolo de corte.
- Modificar: `docs/PLAN-RUST.md`, decisión 4, reflejando protocolo de pot.js al cerrar F7.
- Sin tocar: `src/`, `test/`, `.env` y `start.bat`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

Variables de cada lanzador de prueba. Ganan sobre `.env`:

| Variable | Node de prueba | Rust de prueba |
| :--- | :--- | :--- |
| `DISCORD_TOKEN` | token del bot de prueba 2 | token del bot de prueba 1 |
| `CLIENT_ID` | vacía (sale del token) | vacía (sale del token) |
| `GUILD_ID` | ID del servidor de prueba, o vacía | igual |
| `DATA_DIR` | `..\pruebas-f7\node-data` | `..\pruebas-f7\rust-data` |
| `BIN_DIR` | `bin` | `..\pruebas-f7\rust-bin` |
| `API_PORT` | 3001 | 3002 |
| `AUTO_TUNNEL` | `false` | `false` |
| `PUBLIC_API_URL` | vacía | vacía |
| `DEBUG` | `true` | `true` |

`POT_GENERATOR` no cambia. Node lo usa como en producción, y Rust no arranca pot.js (S-21).

Salida de logs: `..\pruebas-f7\logs\node.log` y `rust.log`. Sin terminal, Node escribe una línea JSON por evento (`logger.js`). Rust debe escribir lo mismo.

Medición:

- `medicion.csv`: columnas `hora`, `bot`, `cpu_pct` y `ram_mb`, cada 60 segundos. La CPU es la variación de `TotalProcessorTime` entre muestras, dividida entre el tiempo transcurrido y los núcleos lógicos. La RAM es `WorkingSet64`. Ambas suman el proceso del bot y sus hijos `ffmpeg.exe` y `yt-dlp.exe`, por `ParentProcessId`.
- `resumen.md`: tabla final con las métricas de la sección Comportamiento, calculada desde los logs y el CSV.

Requisitos de logs. Estas son las líneas que mide la prueba. Rust debe emitirlas con el mismo texto y los mismos campos:

| Medida | Línea de Node | Nivel |
| :--- | :--- | :--- |
| Búsqueda y hasta el audio | `Playing "<nombre>" from <fuente>: found in X s, audio after Y s` | info |
| Corte | `[<cola>] "<nombre>" finished prematurely at Xs of Ys` | warn |
| Pista que no suena | `[<cola>] <nombre>: <detalle>` | error |
| Salida de yt-dlp | `[yt-dlp stderr] <texto>` | warn |
| Latencia de Groq | `Voice input transcribed by Groq Whisper`, con `elapsedMs` | info |
| Fallo de Groq | `Groq transcription request failed`, con `status` | warn |

Un 403 es una línea de error o warn que contiene `403` o `Forbidden`. Un bot-check es una línea que contiene `Sign in to confirm`. Se cuentan por separado. Si una spec de F3 o F4 no emite alguna de estas líneas, se añade antes del corte.

## Comportamiento

1. Carga. `carga.ps1` llama a la API de cada bot cada 10 minutos, con `X-User-Discord-Id` del operador. Busca cinco consultas fijas con `GET /api/v1/search` y reproduce una con `POST /api/v1/guilds/<servidor>/play`. Las dos cargas usan las mismas consultas, con las llamadas escalonadas (ver Decisiones).
2. Medición. `medir.ps1` corre durante las 24 horas. Al terminar, `resumen.ps1` escribe `resumen.md` con estas métricas:

| Métrica | Cálculo |
| :--- | :--- |
| CPU | Mediana y p95 de `cpu_pct` |
| RAM | Mediana y máximo de `ram_mb` |
| Cortes | Líneas de corte por hora |
| 403 | Líneas con `403` o `Forbidden` por hora |
| Bot-check | Líneas con `Sign in to confirm` por hora |
| Pistas que no suenan | Líneas de error de pista por hora |
| Latencia de Groq | Mediana y p95 de `elapsedMs` |
| Búsqueda de yt-dlp | Mediana y p95 de `found in` (S-15) |

3. Corte, en este orden:
   1. Parar Node de producción con Ctrl+C en `start.bat`. Si cmd pregunta si terminar el trabajo por lotes, responder `S`. No cerrar la ventana: Node solo maneja SIGINT y SIGTERM (S-21). Su parada también para `cloudflared`.
   2. Arrancar Rust de producción con `start-rust.bat`, con el mismo `.env`. La música que sonaba con Node se corta, y Rust no restaura sesiones hasta F8 (S-19).
   3. Comprobar: el bot aparece en línea, `bot_status` tiene la URL nueva si `AUTO_TUNNEL` está activo, el dashboard carga, una canción suena y un comando de voz responde.
4. Vuelta atrás: parar Rust con Ctrl+C y arrancar `start.bat`. Node restaura sus sesiones al arrancar (S-19). La vuelta atrás se ensaya antes del corte (ver Tests).

## Tests

Antes de empezar:

- `cargo test` y `npm test` pasan sin cambios.
- Pruebas de paridad automáticas: configuración (S-01), rutas v1 (S-27), SSE (S-28), Supabase con respuestas grabadas (S-30) y palabra de activación con los 11 WAV de `test/fixtures/` (S-06).

Lista de paridad (manual, en el servidor de prueba, con los dos bots):

| # | Área | Qué se comprueba | Spec |
| :--- | :--- | :--- | :--- |
| 1 | Configuración | Mismo `.env`, mismos valores y mismos errores de arranque. | S-01 |
| 2 | Binarios | Sin yt-dlp o sin ffmpeg, el arranque falla con código 1. Con ellos, conecta. | S-32 |
| 3 | Voz recibida | songbird recibe y descifra la voz de una persona (regla de parada, plan §3). | S-07, S-08 |
| 4 | Palabra de activación | Los 11 WAV dan los mismos veredictos que Node. | S-05, S-06 |
| 5 | Comandos de voz | "ignite pausa" pausa la música. Las órdenes de S-10 hacen lo mismo que en Node. | S-09, S-10 |
| 6 | Respuesta de voz | La respuesta por TTS usa la voz y la plantilla de Node. | S-11 |
| 7 | Registro de voz | `voice.log` muestra las entradas. El CSV solo se escribe si `fijar_csv` está activo. | S-12 |
| 8 | Fuentes | Suenan un tema de YouTube, un enlace de Spotify y un enlace directo. | S-13, S-14 |
| 9 | Cola | Añadir, quitar, mover, saltar y deshacer funcionan igual. | S-17 |
| 10 | Filtros | Los 8 filtros suenan y la pista sigue desde el segundo actual. | S-18 |
| 11 | Sesiones | Tras reiniciar Node, la música vuelve. Rust no la restaura hasta F8. | S-19 |
| 12 | Presencia | Pausa al vaciarse el canal. Sale tras el tiempo de S-20. | S-20 |
| 13 | Comandos de barra | Los 25 comandos tienen los mismos nombres, opciones y límites. | S-22 |
| 14 | Permisos | Un usuario fuera de voz no manda sobre la música. | S-23 |
| 15 | Panel | Botones, menú de filtros y modal de volumen funcionan. El panel no se duplica. | S-24 |
| 16 | Vistas | Cola, letras, búsqueda, ayuda, idioma y playlists. | S-25 |
| 17 | API | Las rutas responden como las respuestas grabadas de Node. | S-26, S-27 |
| 18 | SSE | `player.sync` llega al conectar y tras cada cambio. | S-28 |
| 19 | Túnel | Con `AUTO_TUNNEL`, la URL llega a `bot_status` y el dashboard conecta. | S-29 |
| 20 | Datos | Los ajustes de voz se guardan y el ranking coincide con Node. | S-30 |

Medición (24 horas, dos bots, mismas cargas):

- Ventana de 24 horas seguidas, sin reiniciar ninguno de los bots.
- Se cubren los casos "con" y "sin" token renovado: Node con pot.js y Rust sin pot.js, en la misma ventana (S-15).

Ensayo de la vuelta atrás: parar el bot de Rust de prueba y arrancar el de Node de prueba. Comprobar que vuelve a conectar y a restaurar sus sesiones.

Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Los 20 puntos de la lista de paridad están en OK, o con una diferencia aprobada y anotada en `docs/paridad-f7.md`.
- `resumen.md` cubre las 24 horas completas de los dos bots.
- Rust no es peor que Node en CPU, RAM, latencia de Groq, búsqueda de yt-dlp, cortes, 403, bot-check ni pistas que no suenan (el margen de CPU, RAM y latencias lo fija el usuario, ver Preguntas abiertas).
- La decisión sobre pot.js queda anotada en `docs/PLAN-RUST.md`, decisión 4.
- El bot de producción corre en Rust, Node está parado y `start.bat` sigue igual.
- La vuelta atrás se ensayó con los bots de prueba.
- Ningún archivo de `rust/scripts/prueba/` pasa de 250 líneas.

## Preguntas abiertas (Resueltas)

1. Margen de CPU, RAM y latencias: Configurado como comparativa directa en `resumen.ps1`. Cortes, 403, bot-check y errores no tienen margen (Rust <= Node). Para CPU, RAM y latencias, `resumen.ps1` computa el diferencial exacto para validación del operador.
2. Canciones de `carga.ps1`: Fijadas 5 canciones por defecto ("Rick Astley - Never Gonna Give You Up", "Queen - Bohemian Rhapsody", "Lofi Hip Hop Beat Chill", "Ed Sheeran - Shape of You", "The Beatles - Let It Be"), configurables vía el parámetro `-Queries`.
