# Plan de migración de ignite-music a Rust

Rama: `feature/rust-migration`. Este documento es solo el plan. Solo hay un esqueleto de workspace (S-31), sin lógica portada.

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
| 3 | Vosk | Cargar `libvosk.dll` al arrancar con `libloading` | Es lo que hace koffi hoy. Rust usa MSVC (S-31). Usa los archivos de `bin/vosk`. | Crate `vosk`. |
| 4 | YouTube | Solo yt-dlp como proceso externo | Menos código que portar; fuera innertube y relay. En F7 (S-33), si Rust sin pot no supera a Node con pot en 403 ni bot-check, pot.js se retira (F4 a). | Portar el relay (S-15) solo si Rust supera a Node en 403 o bot-check. |
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

Songbird 0.6 dice que soporta DAVE. Su código descifra DAVE al recibir audio (crate `davey`). Eso no prueba voz real. Es el mayor riesgo del plan y se prueba en F2.

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
| S-32 | Binarios y arranque en Windows (yt-dlp, ffmpeg, cloudflared, Vosk, `start.bat`) |
| S-33 | Paridad, medición en paralelo y corte a Rust |

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

## 9. Hoja de ruta

Estado a 2026-11-08. Esta sección amplía la sección 5 con dependencias, puertas, pasos del usuario y decisiones pendientes. Si una decisión cambia, se corrige aquí.

### 9.1 Estado

| Parte | Estado |
| :--- | :--- |
| Plan (secciones 1 a 9) | Escrito. |
| Specs S-01 a S-33 | Escritas. F1 (S-01 a S-04, S-31), F2 (S-07, S-08, S-16), F3 (S-05, S-06, S-09 a S-12), F4 (S-13 a S-15, S-17 a S-20), F5 (S-21 a S-25) y F6 (S-26 a S-30) implementadas. F7: S-32 implementada; S-33 en curso (harness listo, medición operativa pendiente). |
| Commits de specs | Hasta `6d03f2e`. Incluye F1, F6, F7 y F2 (`408a521`, `511b4b5`). |
| Workspace `rust/` (S-31) | Configurado con crates `nucleo`, `bot`, `player`, `voice`, `medios`. |
| Brechas G1 a G4 | Resueltas en S-10, S-17, S-18, S-20, S-21, S-32 y en esta sección. Sin commit. |
| Código de F1 a F8 | F1 Cimientos (30 tests), F2 Prueba de riesgo de voz (19 tests), F3 Comandos de voz (39 tests en voice), F4 Medios y reproductor (31 tests en medios, 41 tests en player), F5 Interfaz de Discord (S-21 a S-25 completadas; hito H4 alcanzado), F6 API y datos (S-26 a S-30 completadas; hito H5 alcanzado), F7 Paralelo y corte (S-32 implementada con binario release; S-33 con harness de medición listo; hito H6 en curso pendiente de validación 24h y corte). F8 pendiente. |
| Node | Sin cambios de este plan. Pasan los 212 tests de Node (check, lint, typecheck, test). |

### 9.2 Hitos

| Hito | Fase | Qué significa |
| :--- | :--- | :--- |
| H1 Voz viva | F2 | songbird recibe voz con DAVE. Es la puerta P1. |
| H2 Voz completa | F3 | La palabra de activación y los comandos dan los resultados de Node. |
| H3 Música completa | F4 | Cola, filtros, sesiones y presencia pasan sus tests. |
| H4 Bot completo | F5 | Comandos y panel funcionan en Discord. |
| H5 Dashboard sobre Rust | F6 | El frontend funciona sin cambios contra Rust. |
| H6 Paridad y corte | F7 | Paridad, 24 horas medidas y token cambiado. Node listo para volver. |
| H7 Node retirado | F8 | Semanas sin volver atrás. Node fuera del código. |

### 9.3 Dependencias

| Fase | Necesita | Puede correr con |
| :--- | :--- | :--- |
| F1 | S-31 | F2 |
| F2 | S-31 | F1 |
| F3 | F1 y F2. F3 crea el trait `Musica` con pausa y reanudación (S-17, G1) | F4 (ver 9.6) |
| F4 | F1 y F2. Eventos simulados hasta F5 para S-20 (G2) | F3 (ver 9.6) |
| F5 | F4, F1 y binarios de Node localizados por variables hasta F7 (G3) | Nada |
| F6 | F4 y F1 | F5, sin bloqueo directo |
| F7 | F1 a F6 y los prerrequisitos de 9.8 | Nada |
| F8 | F7 aprobada y semanas de uso | Nada |

### 9.4 Puertas

| Puerta | Cuándo | Criterio de paso | Si no pasa |
| :--- | :--- | :--- | :--- |
| P1 Voz con DAVE | Tras F2 | songbird recibe voz de una persona con una conexión, en el servidor de prueba | Se para la migración y se elige Salida A o B (sección 3). Lo hecho queda en git. Con Salida B, el código de Rust no se usa. |
| P2 Entrada a F7 | Antes de F7 | F1 a F6 con tests verdes y prerrequisitos de 9.8 | No se empieza F7. |
| P3 Corte | Dentro de F7 | Criterio de aceptación de S-33 | Node sigue en producción. Rust queda en el servidor de prueba. |
| P4 Retiro | Antes de F8 | Semanas sin vuelta atrás y lista de retiro aprobada | Node sigue listo para volver. |

### 9.5 Fases en detalle

#### F1 Cimientos (S-01 a S-04, S-31)

- Objetivo: configuración, errores, i18n y almacén. Son la base de todas las fases.
- Requisitos: workspace de S-31 (esqueleto en `rust/`).
- Entregables: crate `nucleo` con configuración, preflight, token, reporte, errores, registro, errores de API, i18n, almacén, caché y formato. El crate `bot` llama al arranque. Scripts `*-paridad-ref.js` para la referencia con Node.
- Pruebas: `cargo test` de cada spec; `npm run lint` en los scripts; `npm test` sin cambios.
- Salida: con el mismo `.env`, `Config` y `src/config.js` dan los mismos valores (S-01). `ApiError` coincide con Node (S-02). Los 23 idiomas cargan (S-03). El almacén escribe JSON que Node lee, y al revés (S-04).
- Pasos del usuario: decidir `DEV_USER_IDS` y el título de arranque (S-01).
- Revisiones al implementar: nivel `ok` (S-02), regex `\p{Extended_Pictographic}` (S-03), orden de claves con `preserve_order` (S-04).
- Riesgos: el ID de desarrollador fijo activa el modo dev (S-01, S-23).

#### F2 Prueba de riesgo de la voz (S-07, S-08, S-16)

- Objetivo: confirmar que songbird 0.6 recibe voz con DAVE en Windows. Es la puerta P1.
- Requisitos: workspace de S-31. Token de prueba y canal "Tests" del servidor de prueba (ya existen). CMake 4.4.4 solo en `rust/.cargo/config.toml`.
- Versiones fijadas (verificadas en el código de songbird, commit `3f77b5f`): songbird 0.6.0 con `default-features=false` y las features `driver`, `gateway`, `receive`, `tungstenite`, `native` y `twilight`; twilight 0.17; symphonia 0.5.2 con `wav` y `pcm`; davey 0.1.2; opus2 0.4.0. MSRV 1.83.
- Entregables: crate `voice` (`audio.rs`, `receive.rs`, `speakers.rs`, `admission.rs`); crate `player` (`driver.rs`); pruebas `audio_parity.rs`, `receive.rs` y `driver_manual.rs` (esta con `#[ignore]`); `scripts/voice-parity-ref.js`.
- Pruebas: `cargo test` de audio y recepción; `driver_manual` con el token en el servidor de prueba; compilación en Windows.
- Salida: el bot toca música y oye a una persona con una conexión. Compila en Windows.
- Pasos del usuario: hablar en el canal de prueba durante la prueba. Dar el OK para escribir código.
- Riesgos: DAVE no entrega voz (sección 3). Si opus2 no compila, probar `CMAKE_POLICY_VERSION_MINIMUM = "3.5"` (sin probar).
- Nota: S-08 y S-16 necesitan la parte de configuración de S-01 (regla 2 de 9.6).

#### F3 Comandos de voz (S-05, S-06, S-09 a S-12)

- Objetivo: palabra de activación, transcripción, comandos, respuesta por voz y registro de eventos.
- Requisitos: F1 (configuración) y F2 (recepción y driver). Hasta F5, el panel de S-24 es un stub, porque S-10 lo refresca con autoplay.
- Activos existentes: DLL y modelos de Vosk que ya usa Node. Corpus de 11 WAV en `test/fixtures/` del repo de Node. Clave de Groq que ya usa Node.
- Entregables: crate `voice` con S-05, S-06 y S-09 a S-12. En `player`, `musica.rs` con el trait `Musica` y solo la pausa y la reanudación (parte de S-17). Textos fijos en español, por paridad.
- Pruebas: veredictos de S-06 sobre los 11 WAV iguales a Node. S-09 (alucinaciones). S-10 (matcher y despacho). S-12 (anillo de 100 eventos y CSV solo con `fijar_csv`).
- Salida: los mismos veredictos que Node con los audios de prueba. "ignite pausa" pausa la música.
- Pasos del usuario: hablar en el canal de prueba. Confirmar la URL de producción de `IGNITE_SOUND_API_URL` (S-11). Decidir si las frases en español e inglés van juntas o separadas (S-10).
- Decisiones abiertas: ver 9.7.
- Riesgos: la DLL de Vosk trae runtime de MinGW. Confirmar que carga desde un proceso MSVC (S-05). Si no carga, la puerta de palabra queda cerrada (S-06).

#### F4 Medios y reproductor (S-13 a S-15, S-17 a S-20)

- Objetivo: yt-dlp, fuentes, cola, filtros, sesiones y presencia.
- Requisitos: F1 (textos de S-03 y almacén de S-04 para sesiones) y F2 (driver de S-16). Binarios: ver G3. Rust necesita `FFMPEG_PATH` apuntando a `node_modules/ffmpeg-static/ffmpeg.exe`.
- Entregables: crate `medios` con S-13 y S-14 (S-15 no escribe código). En `player`: cola (S-17), filtros (S-18), sesiones (S-19) y presencia (S-20).
- Pruebas: tests portados de cola, filtros y sesiones. Búsquedas y enlaces con yt-dlp. Listas de Spotify con tope de 100.
- Salida: pasan los tests de cola, filtros y sesiones.
- Pasos del usuario: opcional, poner las claves de la API de Spotify para probar esa rama (S-14). Sin claves, S-14 usa el scraping público.
- Riesgos: 403 de YouTube sin relay (sección 8; el relay solo se porta si F7 lo pide, S-15). S-20 usa eventos simulados hasta F5 (G2). Los textos nuevos de S-17 entran en las 23 lenguas (S-03).

#### F5 Interfaz de Discord (S-21 a S-25)

- Objetivo: cliente twilight de un shard, comandos, guards, panel en Components V2 y vistas.
- Requisitos: F4 (cola y presencia), F1 (i18n y errores), F2 (driver). Binarios de Node localizados por variables hasta F7 (G3).
- Entregables: crate `bot` (`cliente.rs`, `apagado.rs` y el arranque en `main.rs`); router de 25 comandos (S-22); guards (S-23); panel (S-24); vistas (S-25, con `formato.rs` en `nucleo`).
- Pruebas: arranque y apagado (S-21); registro de comandos (S-22); tabla de casos de guards (S-23); forma del panel (S-24); vistas (S-25).
- Salida: funcionan todos los comandos y el panel.
- Pasos del usuario: probar comandos y panel en el canal de prueba. Confirmar que Discord acepta el mensaje con Components V2 (S-24).
- Decisiones: el modo dev no se porta en F5 (S-23). Decidir antes de F7 si se porta.
- Riesgos: el modo dev salta la regla de mismo canal (S-23). Los comandos usan el marcador `data/commands-rust.hash`. Si Node y Rust comparten `CLIENT_ID` en F7, cada uno registra la lista completa y gana el último (S-22).

#### F6 API y datos (S-26 a S-30)

- Objetivo: servidor de la API, rutas v1, SSE, túnel y Supabase.
- Requisitos: F4 (cola, reproductor y eventos) y F1 (ApiError y configuración de la API).
- Entregables: `bot/src/api/` con S-26 a S-29. Datos de Supabase con S-30. Migración de `user_favorites` en el repo de Node, fuera de Rust, antes de F7.
- Pruebas: seguridad y capas de la API; rutas v1 contra `tests/fixtures/rutas-node.json`; SSE; túnel con `localhost`; Supabase con respuestas grabadas.
- Salida: el dashboard funciona sin cambios contra Rust.
- Pasos del usuario: confirmar que ningún cliente llama a la API por la IP local (S-26). Decidir `MISSING_USER_ID` (S-27). Fijar el plazo del cambio RLS de voz (S-30). Probar el dashboard contra Rust.
- Riesgos: cualquier cuenta con sesión escribe ajustes de voz (migración `supabase/migrations/20261019_voice_commands.sql`). `openapi.yaml` se corrige en un cambio aparte (S-27). `cloudflared` no se reinicia si muere (S-29).

#### F7 Paralelo y corte (S-32, S-33)

- Objetivo: correr Node y Rust en paralelo 24 horas con dos bots de prueba, medir, y pasar el bot de producción a Rust.
- Requisitos: F1 a F6 terminadas y prerrequisitos de 9.8.
- Entregables: S-32 (`bot/src/binarios/`, `start-rust.bat`, descargas con hash fijado). S-33 (lanzadores de prueba, `medir.ps1`, `carga.ps1`, `resumen.ps1`, `docs/paridad-f7.md` y la decisión 4 de este plan).
- Pruebas: lista de paridad de 20 puntos (S-33); `resumen.md` con las 24 horas; ensayo de la vuelta atrás.
- Salida: criterio de aceptación de S-33.
- Pasos del usuario: los prerrequisitos de 9.8. Probar cada punto de la lista de paridad en Discord. Autorizar el corte.
- Riesgos: los dos bots comparten IP (S-33). YouTube puede dar 403 (S-15). pot.js se decide con datos (S-33). El túnel no se reinicia (S-29).

#### F8 Retiro de Node

- Objetivo: quitar Node cuando la migración lleve semanas en uso sin vuelta atrás.
- Requisitos: puerta P4 y aprobación de la lista de retiro de abajo.
- Entregables: la lista de retiro. Cada punto es un cambio aparte, con OK del usuario.
  1. Código: se quitan `src/`, `index.js`, `shard.js` y `test/`. Se conservan `languages/` (S-03), `data/` (S-04), `bin/`, `supabase/` y `openapi.yaml`.
  2. Scripts: se quitan `scripts/check-syntax.js`, `scripts/patch-voice.js`, `scripts/deploy-commands.js` (lo sustituye S-22) y `scripts/install-binaries.js` (lo sustituye S-32). Los `scripts/*-paridad-ref.js` quedan sin uso, y sus datos en `tests/data/` pasan a ser fixtures. `scripts/mint-pot.js` depende de la decisión sobre pot.js (S-15).
  3. Paquetes: se quitan `package.json`, `package-lock.json`, `node_modules/`, `eslint.config.js` y `tsconfig.json`.
  4. Arranque: `start.bat` lanza el binario de Rust. Se quitan `start-shard.bat` y `start-rust.bat` (S-32). `setup.bat` se reescribe, porque hoy hace `npm install` y copia `.env.example`.
  5. Configuración, CI y docs: `YTDLP_JS_RUNTIME` queda vacía o se quita (S-13). Se quita `.github/workflows/ci.yml`, y `rust.yml` se queda. `README.md` y `AGENTS.md` se reescriben para Rust.
  6. Antes de borrar nada: pot.js no necesita Node (S-15 y decisión 4 de la sección 2). Los contratos congelados (decisión 8 de la sección 2) no cambian.
- Salida: semanas sin vuelta atrás y `AGENTS.md` actualizado.
- Pasos del usuario: decidir cuándo. Aprobar la lista de retiro.
- Riesgos: quitar Node demasiado pronto elimina la vuelta atrás. Por eso existe P4.

### 9.6 Trabajo en paralelo y brechas

Reglas:

1. F1 y F2 pueden correr en paralelo (sección 5). Antes, commit del workspace de S-31. F1 escribe en `nucleo` y `bot`. F2 escribe en `voice` y `player`.
2. S-08 y S-16 necesitan el token y las variables de voz de S-01. Esa parte de S-01 va primero. El resto de F1 sigue en paralelo.
3. F3 y F4 pueden correr en paralelo si el trait `Musica` se acuerda antes (G1).
4. El `Cargo.lock` puede dar conflictos al unir ramas. Se resuelve regenerándolo.
5. Si F2 falla y se elige Salida B, no se usa nada de Rust. El trabajo de F1 hecho en paralelo es el coste.

Brechas entre specs:

| Brecha | Descripción | Resolución | Antes de |
| :--- | :--- | :--- | :--- |
| G1 | S-10 usa un trait `Musica` que S-17 y S-18 no definen | Resuelta. El trait está en S-17 (`musica.rs`), con métodos y errores. S-10 lo usa y declara la dependencia de `player`. F3 implementa pausa y reanudación; F4, el resto. | F3 |
| G2 | S-20 recibe eventos de voz reales solo con S-21 (F5) | Resuelta. Los tests de F4 usan eventos simulados (S-20). S-21, paso 6.2, los conecta en F5. | F4 |
| G3 | S-21 arranca con los binarios de S-32. S-13 y S-18 usan binarios que S-32 garantiza. El plan pone S-32 en F7 | Resuelta. Hasta F7, Rust usa los binarios de Node (S-21, decisión 7; S-32). Para ffmpeg, `FFMPEG_PATH` apunta a `node_modules/ffmpeg-static`. S-32 se implementa en F7. | F4 |
| G4 | F8 no tiene spec | Resuelta. Lista de retiro de seis puntos en F8 (sección 9.5). Sin spec nueva. | F8 |

### 9.7 Decisiones abiertas

| Fase | Decisión | Quién decide | Propuesta de la spec |
| :--- | :--- | :--- | :--- |
| F1 | `DEV_USER_IDS` | Usuario | Sin propuesta |
| F1 | Título de arranque | Usuario | Sin propuesta |
| F1 | Nivel `ok`, regex y `preserve_order` | Agente, al implementar | Revisar (S-02, S-03, S-04) |
| F3 | URL de producción de `IGNITE_SOUND_API_URL` | Usuario | Confirmar el valor (S-11) |
| F3 | Frases en español e inglés juntas o separadas | Usuario | Portar la tabla junta hasta que se decida (S-10) |
| F5 | Modo dev | Usuario | No portar en F5. Decidir antes de F7 (S-23) |
| F5 | Panel Components V2 en Discord | Usuario | Probar en el canal de prueba (S-24) |
| F6 | `MISSING_USER_ID` | Usuario | Responder RFC 7807 si se alcanza (S-27) |
| F6 | Plazo del cambio RLS de voz | Usuario | Sin plazo fijado (S-30) |
| F7 | Margen de CPU y RAM | Usuario | En porcentaje (S-33) |
| F7 | Cinco canciones de la carga | Usuario | Fijas (S-33) |
| F7 | Hashes de ffmpeg, cloudflared y Vosk | Agente, con OK | Fijar tras revisar (S-32) |
| F7 | Versión de Node a comparar | Usuario | Decidir antes. Hay cambios sin commit |

### 9.8 Prerrequisitos antes de F7

- Un segundo bot de prueba y dos canales de voz en el servidor de prueba.
- Migración de `user_favorites` en el repo de Node (S-30). El agente la escribe con OK.
- Margen de CPU y RAM. Cinco canciones para la carga (S-33).
- Hashes de S-32 fijados tras revisión.
- Versión de Node a comparar. Hoy hay cambios sin commit del usuario.
- Plazo del cambio RLS de voz (S-30) fijado antes del corte.

### 9.9 Ritmo

- El plan no fija fechas. Las pruebas en Discord y las decisiones del usuario marcan el ritmo.
- Un módulo por vez (sección 6), salvo las reglas de 9.6.
