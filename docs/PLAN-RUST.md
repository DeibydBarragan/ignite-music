# Plan de migración de ignite-music a Rust

Rama: `feature/rust-migration`. Este documento es solo el plan. No hay código de la migración todavía.

## 1. Alcance

- Portar `ignite-music` de Node.js a Rust, módulo por módulo.
- Hoy son unas 11.900 líneas en `src/` y 3.600 líneas de tests.
- El servicio corre en una máquina Windows. Un solo servidor de Discord usa el bot.
- La fase de medición (antigua F0 de medir CPU, RAM y errores 403) se omitió a pedido del usuario. Las métricas se toman en F7, con Node y Rust en paralelo.

## 2. Decisiones

| # | Decisión | Elección | Motivo | Plan B |
| :--- | :--- | :--- | :--- | :--- |
| 1 | Librería de Discord | twilight 0.17 | El panel usa Components V2. twilight lo trae. serenity 0.12.5 no. | Esperar la rama nueva de serenity. |
| 2 | Voz | songbird 0.6 | Trae DAVE, el cifrado de voz que Discord exige desde marzo de 2026. | Ver la regla de parada (sección 3). |
| 3 | Vosk | Cargar `libvosk.dll` al arrancar con `libloading` | Es lo que hace koffi hoy. Sirve con el toolchain GNU de Rust y usa los archivos de `bin/vosk`. | Crate `vosk`. |
| 4 | YouTube | Solo yt-dlp como proceso externo | Hay menos código que portar. Quedan fuera innertube, relay y pot.js. | Portar el relay solo si aparecen errores 403. |
| 5 | Procesos | Uno solo, con un solo shard. La API (axum) corre dentro del bot. | Un servidor usa el bot. Desaparecen `shard-proxy` y `broadcastEval`. | Separar la API después de F8. |
| 6 | Filtros de audio | Mantener ffmpeg con los 8 filtros de hoy | El sonido queda igual. | DSP propio más adelante. |
| 7 | Sistema | Solo Windows | El servicio corre en la máquina del usuario. | Linux si algún día hay VPS. |
| 8 | Datos compartidos | Congelar `openapi.yaml`, los eventos SSE, los JSON de sesión y el esquema de Supabase | El frontend y los datos actuales siguen sirviendo sin cambios. | Ninguno. |

### Pérdida aceptada

Con un solo proceso, reiniciar el bot corta la música. Hoy pasa igual. Al arrancar, el bot vuelve a entrar con la restauración de sesiones. La música sin cortes pide dos procesos y queda para después de F8.

## 3. Regla de parada

DAVE cifra la voz. Para oír a una persona, el bot debe descifrar su audio:

```text
Voz cifrada -> songbird descifra -> Opus a audio -> Vosk -> Groq
```

Songbird 0.6 dice que soporta DAVE. Sus notas no dicen si funciona al recibir audio. Es el mayor riesgo del plan y se prueba en F2.

Si songbird no recibe voz con DAVE:

- Se detiene la migración antes de gastar más. Lo hecho no se pierde.
- Salida A: la voz se queda en Node y el resto pasa a Rust.
- Salida B: todo se queda en Node con las mejoras de la sección 7.

## 4. Módulos y specs

Solo se nombran. Cada spec se escribe antes de su fase.

| Módulo | Código Node de hoy | Specs |
| :--- | :--- | :--- |
| M1 Núcleo | `config`, `setup/`, `core/` | S-01 a S-04 |
| M2 Voz | `voice/` | S-05 a S-12 |
| M3 Medios | `plugins*`, `innertube`, `relay`, `ytdlp` | S-13 a S-15 |
| M4 Reproductor | `manager`, `playback`, `events`, `session`, `filters`, `presence` | S-16 a S-20 |
| M5 Discord | `bot.js`, `commands/`, `handlers/`, `ui/` | S-21 a S-25 |
| M6 API | `api/` | S-26 a S-29 |
| M7 Datos | `supabase`, `settings-store` | S-30 |
| M8 Operación | `scripts/`, `bin/`, `start.bat` | S-31 a S-33 |

### Lista de specs

| Spec | Nombre |
| :--- | :--- |
| S-01 | Config y arranque (`.env`, preflight, errores de arranque) |
| S-02 | Logs y errores (tracing, ApiError, RFC 7807) |
| S-03 | i18n (las 23 lenguas y validación de claves) |
| S-04 | Almacén atómico y caché TTL |
| S-05 | Binding de Vosk |
| S-06 | Wake-gate (los mismos veredictos que Node) |
| S-07 | Audio (decodificar Opus, bajar de 48k a 16k, RMS, recorte) |
| S-08 | Recepción de voz (DAVE, corte por silencio, filtro de hablantes, admisión y cooldown) |
| S-09 | Cliente de Groq y filtro de alucinaciones |
| S-10 | Matcher y despacho de comandos de voz |
| S-11 | Respuesta por TTS |
| S-12 | Registro de eventos de voz y CSV |
| S-13 | yt-dlp (proceso externo, semáforo, errores) |
| S-14 | Fuentes (búsqueda en YouTube, Spotify, enlaces directos) |
| S-15 | Salida de innertube, relay y pot.js |
| S-16 | Driver de songbird (unirse, tocar, pausar, saltar) |
| S-17 | Cola con bloqueo por guild (reemplaza a `withPlayLock`) |
| S-18 | Filtros de audio con ffmpeg |
| S-19 | Sesiones y restauración al arrancar |
| S-20 | Presencia y salida automática |
| S-21 | Cliente twilight (un shard) |
| S-22 | Slash commands y su registro |
| S-23 | Guards y permisos |
| S-24 | Panel en Components V2 (debounce de 750 ms) |
| S-25 | Vistas de cola, letras, búsqueda, ayuda y playlists |
| S-26 | Servidor axum y seguridad (auth timing-safe, `X-User-Discord-Id`, límite de peticiones, CORS) |
| S-27 | Rutas v1 (player, queue, discovery) según `openapi.yaml` |
| S-28 | SSE por guild |
| S-29 | Túnel de Cloudflare |
| S-30 | Supabase (ajustes de voz, wake words, historial) |
| S-31 | Workspace de Cargo y CI (fmt, clippy, test) |
| S-32 | Instalación de binarios en Windows (Vosk, yt-dlp, ffmpeg) |
| S-33 | Tests de paridad entre Node y Rust |

### Qué lleva cada spec

- Archivos que toca.
- Contrato de entrada y salida.
- Tests que deben pasar.
- Límite de 250 líneas por archivo, igual que la regla de `AGENTS.md`.

## 5. Fases

| Fase | Specs | Criterio de cierre |
| :--- | :--- | :--- |
| F0 Decisiones y contratos | ninguna | Decisiones de la sección 2 escritas. Contratos de la decisión 8 congelados. |
| F1 Cimientos | S-01 a S-04, S-31 | El bot arranca con el mismo `.env` y el i18n pasa la validación. |
| F2 Prueba de riesgo de la voz | S-07, S-16, S-08 | En un servidor de prueba, el bot toca música y escucha voz con una sola conexión. Se confirma que compila en Windows. |
| F3 Comandos de voz | S-05, S-06, S-09 a S-12 | Mismos veredictos que Node con los audios de test. Decir "ignite pausa" pausa la música. |
| F4 Medios y reproductor | S-13 a S-15, S-17 a S-20 | Pasan los tests portados de cola, filtros y sesiones. |
| F5 Interfaz de Discord | S-21 a S-25 | Funcionan todos los comandos y el panel. |
| F6 API y datos | S-26 a S-30 | El frontend funciona sin cambios contra Rust. |
| F7 Paralelo y corte | S-32, S-33 | Se cumple la lista de paridad. Se miden CPU, RAM, cortes, errores 403 y latencia de Groq. Se cambia el token y Node queda listo para volver. |
| F8 Retiro de Node | ninguna | Pasan semanas sin volver atrás. `AGENTS.md` queda actualizado. |

F2 puede empezar en cuanto exista el workspace de S-31. No espera al resto de F1. Si F2 falla, se aplica la regla de parada.

## 6. Cómo se trabaja

- Un módulo por vez. No empieza el siguiente hasta que los tests del anterior pasen.
- Se escribe la spec de una fase justo antes de ella, no todas de golpe.
- Un modelo fuerte decide F2 y revisa el plan. Las specs y el código pueden salir con un modelo más barato.
- Cada fase se puede correr como spec con `task_run`, o como plan completo con `workflow_run`.
- Node sigue desplegable hasta cerrar F8.

## 7. Mejoras

1. **Wake-gate fuera del hilo principal.** `admission.js` llama a `wakeGate.evaluate()` de forma síncrona y frena el event loop. En Node se arregla ya con `koffi .async`. En Rust se usa `spawn_blocking`.
2. **Métricas por etapa** (gate, Groq, yt-dlp, inicio de pista). Sirven para decidir el corte con números.
3. **Tests de contrato contra `openapi.yaml` en CI**, para Node y Rust.
4. **Menos código que portar.** Revisar si innertube, relay y pot.js siguen haciendo falta.
5. **yt-dlp con versión fija** y actualización programada.
6. **Cola guardada en Supabase** para recuperarla tras un reinicio.
7. **Autenticación y límite de peticiones** revisados en la API REST.

## 8. Riesgos

| Riesgo | Efecto | Qué se hace |
| :--- | :--- | :--- |
| songbird no recibe voz con DAVE | Sin comandos de voz en Rust | Prueba en F2 y regla de parada. |
| Compilar Opus en Windows | Pide `cmake` y un compilador de C | Se revisa en F2. |
| Errores 403 de YouTube sin relay | Canciones que no cargan | Se mide en F7. Se porta el relay si hace falta. |
| Falla la voz tras el corte | El bot no escucha | Reactivar Node. |
| Cambios en Components V2 de twilight | Panel roto | Spec S-24 con tests de la forma del mensaje. |
