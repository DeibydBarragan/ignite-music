# Lista de Paridad y Validación F7 (Node vs Rust)

Documento de seguimiento de paridad funcional entre la implementación histórica de Node.js y la versión migrada a Rust (`ignite-music`), según la especificación **S-33** y el plan de migración (`docs/PLAN-RUST.md`).

---

## 1. Pruebas Automáticas de Paridad (Verificadas en CI / Offline)

Las siguientes áreas cuentan con suites de pruebas unitarias y de integración que comparan directamente el comportamiento y las estructuras de datos de Rust contra las referencias históricas o fixtures de Node.js:

| Suite / Área | Archivo de prueba | Cobertura | Estado |
| :--- | :--- | :--- | :--- |
| **Configuración (S-01)** | `rust/crates/nucleo/tests/config_paridad.rs` | 23 variables, prefijos, rangos y defaults | ✅ Verificado |
| **Vosk / STT local (S-05)** | `rust/crates/voice/tests/vosk.rs` | Transcripción y normalización de texto | ✅ Verificado |
| **Palabra de activación (S-06)** | `rust/crates/voice/tests/wake_gate.rs` | 11 fixtures WAV idénticos a Node (`test/fixtures/`) | ✅ Verificado |
| **Matcher de órdenes (S-10)** | `rust/crates/voice/tests/matcher.rs` | 8 intenciones, alias en inglés y español | ✅ Verificado |
| **API Rutas v1 (S-27)** | `rust/crates/bot/tests/rutas_v1.rs`, `rutas_v1_adicionales.rs` | Rutas REST, códigos HTTP y contratos JSON | ✅ Implementado |
| **Eventos SSE (S-28)** | `rust/crates/bot/tests/sse.rs`, `sse_rutas.rs` | `player.sync`, heartbeat y formato de eventos | ✅ Implementado |
| **Supabase / Datos (S-30)** | `rust/crates/bot/tests/datos.rs` | Esquema REST, transacciones y fixtures grabados | ✅ Verificado |
| **Sonda y descarga binarios (S-32)** | `rust/crates/bot/tests/binarios_*.rs` | 19 tests: descarga atómica, SHA-256 y sondas | ✅ Verificado |

---

## 2. Lista de Paridad Funcional (Servidor de Prueba Discord)

Evaluación en servidor de Discord de prueba con los dos bots levantados en paralelo (`lanzar-node.bat` en puerto 3001 y `lanzar-rust.bat` en puerto 3002).

> **Nota de Auditoría F7**: Los módulos y contratos en Rust están implementados y verificados con tests automáticos. La columna **Verificación en Discord** requiere la sesión activa del operador con los 2 bots conectados en paralelo para completar el checklist operativo.

| # | Área | Qué se comprueba | Spec | Código Rust | Verificación en Discord (2 bots) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| 1 | Configuración | Mismo `.env`, mismos valores y errores de arranque | S-01 | ✅ Implementado | ⏳ Pendiente sesión 2 bots |
| 2 | Binarios | Sin yt-dlp/ffmpeg falla 1; con ellos conecta | S-32 | ✅ Implementado | ⏳ Pendiente sesión 2 bots |
| 3 | Voz recibida | songbird recibe y descifra la voz con DAVE | S-07, S-08 | ✅ Implementado | ⏳ Pendiente prueba canal voz |
| 4 | Activación | Los 11 WAV dan los mismos veredictos que Node | S-05, S-06 | ✅ Implementado | ⏳ Pendiente prueba en vivo |
| 5 | Órdenes voz | "ignite pausa" pausa música; órdenes S-10 iguales | S-09, S-10 | ✅ Implementado | ⏳ Pendiente prueba en vivo |
| 6 | Respuesta voz | TTS usa la voz y plantilla de Node | S-11 | ✅ Implementado | ⏳ Pendiente prueba en vivo |
| 7 | Registro voz | `voice.log` muestra entradas; CSV según `fijar_csv` | S-12 | ✅ Implementado | ⏳ Pendiente inspección log |
| 8 | Fuentes | Suenan tema YouTube, Spotify y enlace directo | S-13, S-14 | ✅ Implementado | 🟡 Parcial: YouTube sonó y se escuchó en el canal de prueba (2026-11-12); Spotify y enlace directo sin verificar |
| 9 | Cola | Añadir, quitar, mover, saltar y deshacer iguales | S-17 | ✅ Implementado | ⏳ Pendiente comandos cola |
| 10 | Filtros | Los 8 filtros suenan y pista sigue desde posición | S-18 | ✅ Implementado | ⏳ Pendiente prueba 8 filtros |
| 11 | Sesiones | Reiniciar Node restaura; Rust no hasta F8 | S-19 | ✅ Implementado | ⏳ Pendiente reinicio |
| 12 | Presencia | Pausa al vaciarse canal; sale tras tiempo fijado | S-20 | ✅ Implementado | ⏳ Pendiente desconexión |
| 13 | Slash cmds | 25 comandos con mismos nombres, opciones y límites | S-22 | ✅ Implementado | ⏳ Pendiente inspección cliente |
| 14 | Permisos | Usuario fuera de voz no puede alterar la música | S-23 | ✅ Implementado | ⏳ Pendiente prueba permisos |
| 15 | Panel | Botones, menú de filtros y modal de volumen | S-24 | ✅ Implementado | ⏳ Pendiente interacción UI |
| 16 | Vistas | Cola, letras, búsqueda, ayuda, idioma y playlists | S-25 | ✅ Implementado | ⏳ Pendiente vistas embed |
| 17 | API | Rutas responden según respuestas grabadas de Node | S-26, S-27 | ✅ Implementado | 🟡 Parcial: `/health`, búsquedas y `/play` responden OK en ambos bots (2026-11-12); SSE y paneles pendientes |
| 18 | SSE | `player.sync` llega al conectar y en cada cambio | S-28 | ✅ Implementado | ⏳ Pendiente stream SSE |
| 19 | Túnel | Con `AUTO_TUNNEL`, URL llega a `bot_status` | S-29 | ✅ Implementado | ⏳ Pendiente auto-tunnel |
| 20 | Datos | Ajustes de voz se guardan y ranking coincide | S-30 | ✅ Implementado | ⏳ Pendiente Supabase |

---

## 2.1. Registro de evidencia del piloto (2026-11-12)

Sesión de piloto con los dos bots en paralelo sobre el servidor de prueba (guild `1011718919473610863`, canal de voz `1012424911110799370`), tokens de prueba desde `..\pruebas-f7\tokens.env` (sin tocar el `.env` de producción). Notas cronológicas con logs en `..\pruebas-f7\logs\` (`node.log`, `rust.log`, `carga.out.log`, `medir.out.log`):

- **Conexión de ambos bots**: Node conectó (`Logged in ... 1 shard(s) running`), Rust conectó (`Conectado como Tests Bot 👾#9870 · 1 servidores`). `/health` 200 en los puertos 3001/3002.
- **API y búsqueda**: las 5 búsquedas F7 (`GET /api/v1/search`) respondieron OK en ambos bots; `POST /api/v1/guilds/<guild>/play` devolvió `success: true, started: true, position: 0` en ambos.
- **Corrección de audio en Rust (bloqueante)**: `symphonia` estaba compilado solo con `wav`/`pcm`; songbird registra su propio códec Opus, pero no había **demultiplexores** (`mkv`/`isomp4`/`ogg`), así que el probe escaneaba 1 MB y la pista moría con `no suitable format reader found`. Se habilitaron `aac, alac, flac, mp3, vorbis, adpcm, isomp4, mkv, ogg` en `rust/crates/player/Cargo.toml` y `rust/crates/bot/Cargo.toml`, y `cargo build --release -p bot` compiló correctamente.
- **`GUILD_CREATE` no se suscribía en el bucle de eventos** (`cliente.rs`, solo `READY | INTERACTION_CREATE | VOICE_STATE_UPDATE | VOICE_SERVER_UPDATE | RESUMED`): la caché de guilds quedaba vacía siempre, `/api/v1/guilds` devolvía `[]` y el dashboard se quedaba en skeleton. Añadidos `GUILD_CREATE`/`GUILD_UPDATE`; verificado: `/api/v1/guilds` devuelve el guild `1011718919473610863` (Perros mierda, 18 miembros).
- **`icon` devolvía el hash de imagen de Discord en vez de la URL de CDN** (Node devuelve `iconURL()`): el dashboard no podía renderizar la imagen. Rust ahora devuelve `https://cdn.discordapp.com/icons/<id>/<hash>.webp?size=256` (gif si empieza por `a_`).
- **yt-dlp «Sign in to confirm you're not a bot»**: `lanzar-rust.bat` ahora fija `COOKIES_FILE=cookies.txt` por defecto; el preflight confirma `[ OK ] YouTube cookies` y el video `lKf-kDrckr4` (que fallaba) resuelve correctamente con las cookies del repo.
- **Opcion 2 del operador**: borradas las 9 filas de `song_plays` del piloto (2026-11-12T05:20–06:35, guild de prueba, pedidas por el operador). Verificado: 0 filas restantes en `song_plays` hoy.
- **Slider con seek no hacía nada**: `acciones::buscar` solo actualizaba `posicion_segundos` del estado; nunca se ordenaba un `seek_async` al `TrackHandle` de songbird. Corregido en `proveedor_control.rs`: tras `acciones::buscar` ok, se llama `drv.active_track(guild_id).seek_async(Duration::from_secs(s))`.
- **`/control` con «siguiente» (skip) colgado para siempre**: `ejecutar_control_api` mantenía el `Mutex<Cola>` hasta el final de la función y, antes de devolverlo, llamaba `await reproducir_primera_cancion(...)` que vuelve a pedir el mismo mutex → auto-bloqueo permanente; el dashboard mostraba spinner y «Request timed out». Mismo patrón en `comandos/reproducir.rs` (slash command `/play`). Corregido liberando la guardia antes del avance (`drop(g)` / `drop(cola)`); `/control skip` responde en ~0.05 s.
- **La cola se desincronizaba al saltar (reproductor invisible con audio sonando)**: `saltar()` removía la canción actual de la cola y luego, además, el oyente `end` la removía otra vez — llevándose por delante la canción recién encolada → `/player` mostraba `hasQueue=false` mientras el motor seguía sonando. Invariante: **solo el oyente `end` remueve** en fin natural; en parada manual (`saltar`/`anterior`/`parar`) la acción marca `Cola.paros_manuales` y el oyente lo consume y omite su borrado. La canción saltada queda registrada en `canciones_previas` y la reproducción avanza desde el oyente. Verificado: play → encolar 2ª → `/control skip` → `/player` muestra la siguiente con `currentTime` avanzando.
- **Autoplay no encolaba temas relacionados** (DisTube `NO_RELATED` en Node): al vaciarse la cola con autoplay activo, el bot se quedaba en silencio. Implementado en el handler `end` (`ciclo_vida.rs`): al terminar la pista, con cola vacía + autoplay, se toma el ID del vídeo terminado, se busca el mix relacionado vía `medios::relacionados` y se encola y reproduce el primer tema distinto al actual.
- **La posición del track nunca avanzaba** (`posicion_segundos` fija en 0): el dashboard mostraba el progreso y saltaba siempre a 0 s. Añadido `SongbirdDriver::active_track_handles()` y una tarea en `ciclo_vida.rs` que cada 2 s lee `get_info()` de los tracks activos, actualiza `posicion_segundos` y publica un `player.sync` SSE por guild con pistas (cubre clientes con stream en vivo). Verificado en vivo: `currentTime` pasa de 0 a 6 s en 7 s de reproducción.
- **Auto-desconexión «estando yo dentro» (presencia)**: el tracker de auto-salida por canal vacío nunca se enteraba de los usuarios que ya estaban en voz cuando el bot conectaba (solo sembraba con `VOICE_STATE_UPDATE` en vivo), por lo que contaba 0 humanos y a los 60 s (`LEAVE_ON_EMPTY`) cerraba el canal aunque tú estuvieras dentro. Corregido: en `GUILD_CREATE` se siembra `voice_states` del payload del gateway en el tracker (mismo manejo de `es_bot` que los eventos normales). Verificado en el log: `Seeded 1 voice state(s) from GUILD_CREATE for guild 1011718919473610863`.
- **Verificación tras el fix (05:43 UTC)**: `Joining voice channel` → `Joined voice channel successfully`; el probe de symphonia encontró el marcador EBML/Matroska en el byte 2 (`found the format marker [1a, 45, df, a3]`); la pista pasó a `Ready(Playable)` (`Firing Playable for [0]`); **el operador confirmó por voz que se oía bien** en el canal de prueba.
- **Scripts F7 corregidos**: `carga.ps1` envía `Authorization: Bearer <API_SECRET>` y `voiceChannelId` explícito (`TEST_VOICE_CHANNEL_ID`, por defecto `1012424911110799370`), e incluye `DISCORD_OPERATOR_ID`/`TEST_GUILD_ID`; `medir.ps1` apunta correctamente a `..\pruebas-f7\medicion.csv` y mide CPU real; `resumen.ps1` con rutas y percentiles corregidos.
- **Piloto de paridad en 1 hora** (`-DurationHours 1`, decisión del operador — no 2h ni 24h) completado a las 06:43 UTC (0.99 h, **120 muestras**; `medicion.csv` y `resumen.md` en `..\pruebas-f7\`):
  - **CPU**: mediana Node 0 % vs Rust 0.02 %; p95 Node 0.2 % vs Rust 0.08 %.
  - **RAM**: mediana Node 439.75 MB vs Rust 31.68 MB (−408 MB); máximo Node 568.24 MB vs Rust 96.63 MB (−471.6 MB).
  - Cortes, 403, bot-check y pistas sin sonar: **0 /h en ambos** (el «8.05 /h» inicial de Node era un falso positivo del contador —líneas debug `stream error: Premature close` al terminar la pista—; corregido en `resumen.ps1` y regenerado el informe).
  - Todas las rondas de reproducción Rust dentro de la carga (cada 10 min) terminaron con `Reproduccion iniciada: True` y confirmaste por voz que se oye.
  - **Sin medir** (la carga no ejerce esas rutas): Groq STT y tiempos de búsqueda yt-dlp salen 0 en el informe — no cuentan como «cero» real.
  - El informe imprime automáticamente «Decisión pot.js: Aprobada retirada de pot.js» por comparación de 403/bot-check (ambos 0); **es el mensaje canned del script, no evidencia de que la retirada esté validada** — ambos contadores eran 0 porque la carga no provocó respuestas PO-token-sensibles en ninguna de las dos partes.
  - **Observación pendiente en Rust**: ráfagas de `WARN RTCP decryption failed: Crypto(Error)` (~22) y líneas debug `hyper::Error(Shutdown, ConnectionReset)` sincronizadas con cada arranque de pista; el audio funciona, pero hay que investigar (posible descifrado RTCP/DAVE no soportado en songbird; relacionado con el punto #3 de la lista, aún sin verificar).

> **Aún sin verificar**: Spotify y enlace directo (fuentes, #8), paneles/botones/filtros en vivo (#10, #15, #16), restauración de sesión (#11), presencia/auto-desconexión (#12), DAVE con voz real recibida (#3, #4, #5, #6, #7), SSE en vivo (#18), túnel (#19), datos/ranking con Supabase en vivo (#20).

---

## 3. Protocolo de Medición en Paralelo (duración configurable)

1. **Preparación del entorno**:
   - Crear directorio `..\pruebas-f7\` (los scripts lo crean automáticamente).
   - Configurar `TEST_DISCORD_TOKEN_NODE` y `TEST_DISCORD_TOKEN_RUST`.
   - Definir `TEST_GUILD_ID` y `DISCORD_OPERATOR_ID`.

2. **Lanzamiento de los bots**:
   ```cmd
   start "Node Test" cmd /c rust\scripts\prueba\lanzar-node.bat
   start "Rust Test" cmd /c rust\scripts\prueba\lanzar-rust.bat
   ```

3. **Inicio de métricas y carga escalonada**:
   La duración se fija con `-DurationHours` (por defecto 24 en los scripts; el piloto actual usa **1**):
   ```powershell
   Start-Process powershell -ArgumentList "-NoProfile -File rust\scripts\prueba\medir.ps1 -DurationHours 1 -IntervalSeconds 60"
   Start-Process powershell -ArgumentList "-NoProfile -File rust\scripts\prueba\carga.ps1 -DurationHours 1"
   ```

4. **Cierre y generación de informe**:
   Al culminar el piloto, ejecutar:
   ```powershell
   powershell -NoProfile -File rust\scripts\prueba\resumen.ps1
   ```
   Revisar `..\pruebas-f7\resumen.md` y verificar que Rust cumple todos los umbrales.

---

## 4. Ensayo de Vuelta Atrás (Rollback Drill)

Se ensaya con los bots de prueba antes del corte en producción:
1. Parar el proceso de Rust con `Ctrl+C`.
2. Iniciar inmediatamente el proceso de Node con `rust\scripts\prueba\lanzar-node.bat`.
3. Comprobar que Node reconecta a Discord y restaura las sesiones guardadas en disco.

---

## 5. Protocolo de Corte a Producción

1. En la consola de Node de producción (`start.bat`), presionar `Ctrl+C` y responder `S`. Esperar apagado limpio.
2. Iniciar `start-rust.bat` en la raíz del proyecto.
3. Verificar:
   - Bot en línea en Discord.
   - Panel de control y dashboard operativos (`http://localhost:3000` / túnel).
   - Reproducción de canción de prueba.
   - Respuesta a comando de voz o comando de barra `/play`.
