# Revisión de entregas 4 a 7 frente al código

**Fecha:** 19 de noviembre de 2026, entre las 15:12 y las 15:30 (hora local del equipo).
**Rama:** `feature/rust-migration`, 15 commits sobre `main`. HEAD `9d5686d`.
**Árbol de trabajo:** 108 entradas en `git status`, entre modificadas y sin seguimiento.
**Alcance:** solo lectura. No se hizo commit, push, stash, reset ni edición. Esta revisión no decide nada.

Referencias: [plan de cierre](PLAN-CIERRE-MIGRACION-RUST.md) y [auditoría](AUDITORIA-MIGRACION-RUST.md).

## Resumen

- Los gates pasan hoy: formato, Clippy, 319 pruebas Rust, build release, y los cuatro controles de Node.
- Varias afirmaciones del plan no coinciden con el código. Las más graves están en la entrega 5: sesiones, historial y credenciales.
- La entrega 2 ya conecta el receptor de voz en `main.rs`. El hallazgo 1 de la auditoría está parcialmente resuelto.
- Ninguna entrega está cerrada. Las entregas 3, 4 y 6 necesitan prueba en Discord o en el dashboard, que solo una persona puede hacer.
- Casi todo el trabajo de Rust sin commit. El riesgo de pérdida es alto.

## Gates ejecutados

Rust se ejecutó con `C:\Users\Usuario\.cargo\bin\cargo.exe`, porque `cargo` no está en el PATH de esta sesión.

| Gate | Resultado | Nota |
| --- | --- | --- |
| `cargo fmt --check` | Pasa (exit 0) | La auditoría indicaba cuatro archivos con fallo. Ya no aplica. |
| `cargo clippy --all-targets --locked -- -D warnings` | Pasa (exit 0) | La auditoría indicaba un fallo de cast. Ya no aplica. |
| `cargo test --workspace --locked` | 319 pasan, 0 fallan, 2 ignoradas | Las ignoradas son `driver_manual` y `recepcion_manual`. Suma de todos los binarios. |
| `cargo build --release --locked` | Pasa (exit 0) | `rust/target/release/bot.exe` SHA-256 `33DE6E76765FCF27CE45C68AE2BD8E3BA62FD1A1D6E498BF2CC1DC065B988D10`. |
| `npm run check` | Pasa | 180 archivos. |
| `npm run lint` | Pasa | Sin errores. |
| `npm run typecheck` | Pasa | Sin errores. |
| `npm test` | 234 pruebas, 227 pasan, 0 fallan, 7 omitidas | El plan dice 205 pasan. La cifra cambió. |

El hash del `bot.exe` cambia con cada compilación. El plan cita `1132D3...` para la entrega 4 y `aa1581...` para la entrega 3. Ninguno de los dos hashes sirve como evidencia estable mientras el árbol siga cambiando.

Los conteos de pruebas de Rust que aparecen en el plan (286, 289, 290, 293, 303, 305) no siguen una secuencia clara. Cada entrega debería citar un comando y un commit concretos.

## Entrega 1. Referencia reproducible

| Punto | Estado | Evidencia |
| --- | --- | --- |
| Gates de Node y Rust pasan en la máquina local | Hecho | Ver tabla de gates. |
| `rust/rust-toolchain.toml` fija 1.99.1 | Hecho | `rust/rust-toolchain.toml:2`. |
| Mínimo de Rust igual a las dependencias resueltas y probado | Pendiente | `rust/Cargo.toml:8` declara `rust-version = "1.99.1"`. La auditoría citaba 1.83 y dependencias que requieren 1.89. No hay prueba con el mínimo real. |
| Workflow de CI incluye fixtures, idiomas, WAV y scripts | Parcial | `.github/workflows/rust.yml` no tiene seguimiento en Git. No se revisó el filtro de rutas. |
| Activos de prueba bajo `rust/crates/*/tests/data/` incluidos en Git | Pendiente | El plan dice que se incluyen. `.gitignore` y `git ls-files` no se revisaron en esta pasada. La auditoría dice que 24 JSON locales están ignorados y que una copia limpia falla al compilar. |
| CMake configurable por variable o detección | No verificado | Revisar `rust/.cargo/`. |
| Una copia limpia en Windows construye igual | Pendiente | No se hizo. |
| Conciliar S-14 sin claves y S-19 | Pendiente | Decisión del operador. |

**Conclusión:** la entrega 1 no está cerrada. El punto de mínimo de Rust y la copia limpia faltan.

## Entrega 2. Gateway y recepción DAVE

| Punto | Estado | Evidencia |
| --- | --- | --- |
| El receptor de voz se conecta a las llamadas de Songbird | Hecho en código | `rust/crates/bot/src/main.rs:111` llama a `preparar_recepcion`. `rust/crates/bot/src/recepcion_voz.rs:46-54` registra `VoiceReceiverHandler`. `rust/crates/player/src/driver_conexion.rs:31-51` lo aplica al crear la conexión. |
| Los fragmentos llegan al procesador de voz | Hecho en código | `rust/crates/bot/src/main.rs:187` inicia `comandos_voz::iniciar` con el canal de fragmentos. |
| Diferir `/play` antes de esperar voz | No confirmado | `rust/crates/bot/src/comandos/` tiene `defer` en `buscar.rs`, `letras.rs`, `listas.rs` y `listas_crear.rs`. No aparece en `reproducir.rs`. La auditoría (hallazgo 2) dice que sigue abierto. |
| El gateway no bloquea mientras corren interacciones largas | No confirmado | `rust/crates/bot/src/cliente.rs:164-182` tiene `spawn` para ready y trabajos. No se leyó el bucle completo. |
| Prueba automática del adaptador con gateway y driver controlados | Pendiente | No hay prueba con esas características. `rust/crates/bot/tests/recepcion_manual.rs` está ignorada. |
| P1 en Discord real | Pendiente | Solo una persona con Discord puede cerrarlo. |
| Supervisar la tarea del gateway | Pendiente | La auditoría (S-21) dice que `main` espera una señal aunque la tarea haya fallado. `main.rs:238` sigue el patrón sin revisión adicional. |

**Conclusión:** la conexión del receptor existe. Falta el defer de `/play`, la prueba automática y P1.

## Entrega 3. Comandos de voz

| Punto | Estado | Evidencia |
| --- | --- | --- |
| Cadena fragmento, RMS, compuerta, cooldown, Groq/Vosk, matcher y música | Hecho en código | `rust/crates/bot/src/comandos_voz.rs` (189 líneas) conecta gate, admisión, matcher y registro. |
| Reconocimiento nativo en trabajo bloqueante | Hecho en código | `comandos_voz.rs:41` usa `spawn_blocking`. |
| `gate_unavailable` cuando hay gating y falta evaluador | Pendiente | El literal solo aparece en `rust/crates/bot/src/comandos_voz/pruebas.rs:101`. En `comandos_voz.rs:163-164` el código emite `gate_disabled` cuando no hay evaluador. |
| Reserva de cooldown atómica por guild y usuario | No verificado | No se leyó `voice/src/admission.rs`. |
| Tipos propios para handles nativos (Vosk) | No verificado | La auditoría (S-05) dice que circulan como `usize`. |
| TTS, `voice.log` y CSV conectados a eventos reales | Parcial | `comandos_voz.rs:186` llama a `registro.registrar`. El TTS y el CSV no se verificaron. |
| Corpus de 33 combinaciones en verde | Hecho | `wake_gate.rs` pasó 5 pruebas. No se contó el corpus exacto. |
| Discord real: `ignite pausa` cambia estado REST y handle | Pendiente | Requiere prueba en vivo. |
| Groq caído cae a Vosk, compuerta cerrada no llama a Groq | Parcial | Hay pruebas con doble. No se ejecutó contra un bot real. |

**Conclusión:** el procesador está conectado. Faltan `gate_unavailable`, el contador cero de Groq con compuerta cerrada y la prueba en Discord.

## Entrega 4. Medios, cola y fuentes

| Punto | Estado | Evidencia |
| --- | --- | --- |
| Todas las pistas de una lista se encolan | No verificado | Auditoría, hallazgo 4, dice que el slash conserva solo la primera. No se leyó `comandos/reproducir.rs` a fondo. |
| Límite de concurrencia de yt-dlp compartido | Parcial | `rust/crates/bot/src/binarios/mod.rs:142` crea un único `Limitador::new`. `player/src/driver_reproduccion.rs:78` lo recibe por opciones. No se confirmó que búsqueda y extracción reciban la misma instancia. |
| Medición `startup_latency_ms` | Hecho | `rust/crates/player/src/driver_inicio.rs:15`. |
| Módulos `.rs` en `rust/**/src` con máximo 250 líneas | Hecho | Revisados 194 archivos. Ninguno supera 250 líneas. |
| Spotify resuelto a audio de YouTube | No verificado | Auditoría, hallazgo 4, dice que sigue abierto. |
| Filtros con hashes PCM | Hecho | `rust/crates/player/tests/filtros.rs` pasó 6 pruebas. |
| `/skip` y `/previous` no detienen si la cola rechaza la operación | No verificado | Auditoría, hallazgo 3, dice que sigue abierto. |
| Validación en Discord de presets, seek, autoplay, presencia | Pendiente | Requiere prueba en vivo. |
| Medición de inicio llega al historial | Pendiente | Ver entrega 5 sobre historial. |

**Conclusión:** la entrega 4 tiene progreso real en límite de medios y latencia. El plan dice «avance», y eso es correcto. Varias afirmaciones de la sección de cola no se pudieron verificar en esta revisión.

## Entrega 5. Discord, persistencia y recuperación

| Punto | Estado | Evidencia |
| --- | --- | --- |
| Los 25 comandos atraviesan el despachador real | Parcial | `rust/crates/bot/tests/comandos.rs` existe. No se contó cuántos comandos cubre. |
| Sesiones cargadas antes de la primera lectura | Pendiente | `AlmacenSesiones::initialize` está definido en `rust/crates/player/src/sesiones.rs:93`. Ningún código de `rust/crates/bot/src` lo llama. `arranque.rs:38-46` restaura sin inicializar el almacén. Confirma el hallazgo 7. |
| Restauración de sesión mezclada con enlaces directos y genéricos | Pendiente | Auditoría, hallazgo 7: `rust/crates/player/src/sesiones.rs:78` solo guarda claves de YouTube y Spotify. |
| Restauración conserva volumen cero | Pendiente | La auditoría dice que el volumen guardado a cero se sustituye por 80. |
| Historial registra cada escucha | Pendiente | `rust/crates/bot/src/api/proveedor_reproduccion.rs:201` registra solo en `POST /play`. No hay registro en slash, avance de cola ni autoplay. Confirma el hallazgo 11. |
| Credenciales de escritura separadas de lectura | Pendiente | `rust/crates/bot/src/datos/supabase.rs:39-43` usa la clave de servicio si existe, y si no, la clave que llegue. Confirma parte del hallazgo 10. |
| Migración de favoritos en el repositorio | Pendiente | Auditoría, hallazgo 10. No se buscó en `supabase/migrations`. |
| Políticas de voz restringidas al propietario | Pendiente | Auditoría, hallazgo 10: `supabase/migrations/20261019_voice_commands.sql:52` admite escrituras de cualquier cuenta autenticada. |
| Panel: primer envío, ID real del mensaje, acciones | No verificado | Auditoría, hallazgo 8, dice que sigue abierto. |

**Conclusión:** la entrega 5 tiene pendientes concretos y verificables. El plan la trata como progreso, pero las sesiones y el historial no funcionan como se describe.

## Entrega 6. Contrato desplegable y dashboard

| Punto | Estado | Evidencia |
| --- | --- | --- |
| `openapi.yaml` documenta las 30 operaciones | Parcial | `openapi.yaml` tiene 27 claves de ruta. Cada clave puede tener varios métodos, así que el conteo de operaciones no se verificó. `test/openapi.test.js` existe y no tiene seguimiento en Git. |
| SSE con la misma instantánea que REST | Parcial | `rust/crates/bot/src/api/sse.rs:134` usa `estado_inicial` para el primer evento. `sse.rs:162` publica el resto. No se comparó el cuerpo de cada evento con `GET /player`. |
| Ticker de `player.sync` sin cuerpo vacío | No verificado | Auditoría, hallazgo 9. |
| Apagado cierra SSE antes del plazo de diez segundos | Pendiente | Auditoría, hallazgo 9: el hub se cierra después de esperar a Axum. |
| BFF de `ignite-frontend` valida sesión y membresía | Pendiente | Repositorio vecino, no revisado en esta pasada. El plan dice que falta. |
| Recorrido con dashboard conectado a Discord | Pendiente | Requiere prueba en vivo. |
| P2 | Pendiente | Solo una persona puede cerrarlo. |

**Conclusión:** la entrega 6 no está cerrada. El plan marca «avance» y eso es correcto.

## Entrega 7. F7, reversión y corte

| Punto | Estado | Evidencia |
| --- | --- | --- |
| Dos bots aislados con puertos, canales y directorios | Pendiente | No verificado. La auditoría (S-32) dice que el lanzador puede usar el token por defecto. |
| Medición con PID, puerto y ejecutable | Pendiente | Auditoría, hallazgo 12: `rust/scripts/prueba/medir.ps1:33`. |
| 24 horas válidas y 20 puntos aprobados | Pendiente | `docs/paridad-f7.md` registra 0,99 h y 2 de 20 filas. |
| Ensayo de reversión | Pendiente | No hay evidencia. |
| P3 y P4 | Pendiente | Decisión del operador. |

**Conclusión:** la entrega 7 no empezó. Depende de P1 y P2.

## Hallazgos de la auditoría: estado actual

| N.º | Tema | Estado | Evidencia |
| --- | --- | --- | --- |
| 1 | Recepción de voz sin conectar | Parcial | Conectada en `main.rs:111` y `driver_conexion.rs`. Falta P1 y el resto del hallazgo. |
| 2 | `/play` puede bloquear su propia conexión | Abierto | Sin `defer` en `reproducir.rs`. |
| 3 | Controles divergen entre REST, slash, panel y presencia | No verificado | Se mantiene como abierto según la auditoría. |
| 4 | Pistas que no suenan y Spotify sin audio de YouTube | No verificado | Se mantiene como abierto según la auditoría. |
| 5 | Límite de yt-dlp no compartido | Parcial | Una sola construcción en `binarios/mod.rs:142`. Falta confirmar el resto. |
| 6 | Permisos y contexto de canal divergen | No verificado | Se mantiene como abierto según la auditoría. |
| 7 | Restauración no recupera sesión | Abierto | `AlmacenSesiones::initialize` sin llamadas en `bot/src`. |
| 8 | Panel con efecto incompleto | No verificado | Se mantiene como abierto según la auditoría. |
| 9 | SSE con formatos mezclados y apagado | No verificado | Se mantiene como abierto según la auditoría. |
| 10 | API y Supabase con diferencias | Abierto | Clave de servicio con fallback en `datos/supabase.rs:39-43`. Políticas de voz sin cambios. |
| 11 | Historial solo en `POST /play` | Abierto | `proveedor_reproduccion.rs:201` es la única escritura. |
| 12 | Piloto y CI no permiten aprobar F7 | Parcial | Formato y Clippy ya pasan. `rust-version` declara 1.99.1 sin prueba de mínimo. CI sin seguimiento. |

## Premisas dudosas del plan

1. **«Entrega 3 hecha»** según el chat. El código no tiene `gate_unavailable`, y la prueba en Discord no existe. Debe marcarse como parcial.
2. **Conteos de pruebas.** Cada documento cita una cifra distinta. Conviene fijar una sola fuente, el comando y el commit.
3. **Hashes como evidencia.** Cambian con cada compilación. Sirven solo con commit fijo.
4. **«Cuatro gates de Node pasan con 205 pruebas.»** Hoy hay 227 pasando y 7 omitidas. El número de pruebas de Node cambió. Hay que fijar la referencia.
5. **Mínimo de Rust.** El plan pide alinearlo con las dependencias. El `Cargo.toml` solo dice 1.99.1, sin prueba de mínimo.
6. **Sesiones en F7 y F8.** El plan dice que la restauración se hace en el corte y la auditoría (S-19) dice que parte se difiere. Hay que decidirlo antes de implementarlo.
7. **Spotify sin claves.** El plan y S-14 se contradicen. Decisión del operador.
8. **«Migración reproducible de favoritos».** No aparece en el repositorio.

## Entregas que se pisan

| Archivos o carpetas | Entregas |
| --- | --- |
| `rust/crates/bot/src/main.rs`, `arranque.rs` | 2, 5 |
| `rust/crates/bot/src/comandos_voz.rs`, `rust/crates/voice/src/` | 3 (y 2 en el receptor) |
| `rust/crates/player/src/driver.rs`, `driver_conexion.rs`, `driver_reproduccion.rs` | 2, 4 |
| `rust/crates/bot/src/comandos/reproducir.rs`, `rust/crates/bot/src/interacciones_comandos_aux.rs` | 2, 4, 5 |
| `rust/crates/bot/src/api/ciclo_vida.rs`, `api/eventos_reproduccion.rs` | 4, 5, 6 |
| `rust/crates/bot/src/api/proveedor_reproduccion.rs`, `api/proveedor_control.rs`, `api/rutas/` | 4, 5, 6 |
| `rust/crates/bot/src/panel/`, `ui/` | 5, 6 |
| `rust/crates/player/src/sesiones*.rs`, `restauracion.rs`, `datos/supabase.rs` | 5 |
| `rust/crates/bot/src/api/sse.rs`, `panel/instantanea.rs` | 6 |
| `openapi.yaml`, `test/openapi.test.js` | 6 |
| `.github/workflows/rust.yml`, `rust/Cargo.toml`, `rust/scripts/` | 1, 7 |
| `ignite-frontend/` (BFF y hooks) | 6, y el buscador sin commit del usuario |

Entrega 2 y 4 modifican `driver.rs`. Entrega 4 y 6 modifican `ciclo_vida.rs`. Entrega 5 y 4 modifican `proveedor_reproduccion.rs`. Entregas 1 y 7 modifican la CI.

## Riesgo de nada comiteado

- `rust/crates/bot/`, `rust/crates/nucleo/`, `rust/crates/player/`, `rust/crates/voice/`, `rust/crates/medios/` y `rust/Cargo.toml` no tienen seguimiento. Solo dos archivos de `rust/` están en Git.
- Las 108 entradas del árbol mezclan trabajo de la migración con cambios del usuario en `src/`, `test/` y `eslint.config.js`.
- Un `git clean` o un error de checkout borraría el código Rust. Conviene un commit de respaldo en rama antes de seguir.
- No se revisó si hay ramas de respaldo o stash.

## Puntos que solo una persona con Discord puede cerrar

- **P1.** Recepción DAVE en vivo: `/play` en canal frío, diez frases humanas, bot que no admite fragmentos propios.
- **P2.** Recorrido de controles en vivo con el dashboard conectado a un bot de Discord.
- **P3.** Ensayo de 24 horas, cambio de token y decisión de pot.js. Requiere aprobación del operador.
- **P4.** Retiro de Node. Requiere semanas de observación.
- **Entrega 3.** Prueba de «ignite pausa» en Discord.
- **Entrega 4.** Escucha de YouTube, tema, lista Spotify y enlace directo.
- **Entrega 5.** Restauración real tras reinicio con personas presentes.

## Tareas independientes propuestas

Cada tarea incluye su condición de aceptación comprobable. Se recomienda un commit de respaldo antes de empezar.

| ID | Tarea | Archivos | Aceptación | Puede correr en paralelo con |
| --- | --- | --- | --- | --- |
| T1 | Fijar el mínimo de Rust y probarlo | `rust/Cargo.toml`, `rust/rust-toolchain.toml`, `.github/workflows/rust.yml` | `cargo +<mínimo> check --locked --workspace --tests` pasa en una copia limpia. | T3, T4, T5, T7 |
| T2 | Inicializar el almacén de sesiones al arrancar | `rust/crates/bot/src/arranque.rs`, `main.rs`, `rust/crates/player/src/sesiones.rs` | Prueba: un archivo de sesión en disco se restaura tras reiniciar el almacén. | T4, T5, T7 |
| T3 | Registrar historial en slash, avance de cola y autoplay | `rust/crates/bot/src/api/ciclo_vida.rs`, `comandos/reproducir.rs`, `datos/historial.rs` | Prueba: cada inicio de pista llama a `registrar_reproduccion` una vez. | T1, T4, T5, T7 |
| T4 | Exigir credenciales de escritura en Supabase | `rust/crates/bot/src/datos/supabase.rs`, configuración | Prueba: escritura con clave anónima devuelve error; lectura sigue funcionando. | T1, T2, T3, T5, T7 |
| T5 | Añadir `gate_unavailable` y cero llamadas a Groq con compuerta sin evaluador | `rust/crates/bot/src/comandos_voz.rs`, `rust/crates/voice/src/admission.rs` | Prueba: gating activo y evaluador nulo termina con `gate_unavailable` y cero llamadas al doble de Groq. | T1, T4, T7 |
| T6 | Diferir `/play` antes de esperar la unión de voz | `rust/crates/bot/src/comandos/reproducir.rs`, `cliente.rs` | Prueba: la respuesta diferida se envía antes de que `join` termine. | T1, T4, T5, T7 |
| T7 | Cubrir las 30 operaciones en `openapi.yaml` | `openapi.yaml`, `test/openapi.test.js` | `npm test` pasa con inventario completo; Redocly valida el archivo. | T1 a T6 |
| T8 | Mismo payload en SSE y `GET /player` | `rust/crates/bot/src/api/sse.rs`, `panel/instantanea.rs` | Prueba: el evento inicial y el `player.sync` coinciden con `GET /player`. | T7 |

Conflictos: T2 y T6 tocan `main.rs` o `cliente.rs`, así que conviene serializarlos. T3 y T8 tocan `ciclo_vida.rs`, así que van en serie. T1, T4, T5 y T7 no comparten archivos y pueden correr a la vez.

Quedan fuera de estas tareas, porque necesitan una decisión o una persona: la prueba en Discord de P1, P2 y P3, la política de sesiones F7/F8, el tratamiento de Spotify sin claves y la decisión de pot.js.

## Qué no se revisó

- No se leyó el código de controles, panel, presencia ni los 25 comandos fila por fila.
- No se revisó el BFF ni los hooks de `ignite-frontend`.
- No se ejecutaron pruebas contra Supabase ni contra Discord.
- No se revisó el contenido de `supabase/migrations`.
- No se contaron las 33 combinaciones del corpus de voz.
- Riesgo: algunos «No verificado» pueden estar resueltos. Cada uno necesita una lectura de su archivo antes de marcarlo como cerrado.
