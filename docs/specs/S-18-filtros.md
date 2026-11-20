# Spec S-18: Filtros de audio con ffmpeg

- Estado: implementado en el pipeline de reproducción; validación audible en Discord pendiente
- Fase: F4
- Módulo: M4
- Origen en Node (solo lectura): `src/music/filters.js` (`FILTERS`, `FILTER_LABELS`), `src/music/actions.js` (`setFilter`), `node_modules/distube/dist/index.js` (argumentos de ffmpeg de cada filtro, DisTube 5.2.3), `src/music/manager.js` (opciones de entrada de ffmpeg), `test/actions.test.js` (filtros).

## Objetivo

Aplicar uno de los 8 filtros de hoy al audio de la cola, con los mismos argumentos de ffmpeg, para que el sonido salga igual. Es la decisión 6 del plan: se mantiene ffmpeg.

No incluye la cola (S-17) ni la entrada de los flujos (S-14 y S-16).

## Archivos

- Creado: `rust/crates/player/src/filtros.rs` (catálogo, etiquetas y argumentos de ffmpeg)
- Creado: `rust/crates/player/src/filtros_ffmpeg.rs` (proceso ffmpeg y lectura de su salida)
- Creado: `rust/crates/player/src/pipeline_audio.rs` (yt-dlp → ffmpeg → WAV detectado por Songbird)
- Integrado: `rust/crates/player/src/driver_reproduccion.rs` y `rust/crates/player/src/reproduccion.rs`
- Creado: `rust/crates/player/tests/filtros.rs`
- Creado: `rust/crates/player/tests/data/filtros-casos.json` (generado con Node)
- Creado: `scripts/filtros-paridad-ref.js` (pasa `npm run lint`)
- No tocar: `src/music/filters.js`, `src/music/actions.js`, `node_modules/distube/`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada de `fijar_filtro(cola, nombre)`: nombre del filtro, `none` para quitarlo, o vacío.
- Salida: `Resultado { ok, mensaje }`.
- Errores: un nombre que no está en el catálogo da `ok: false` con la clave `errors.unknown`.

## Comportamiento

1. Catálogo, en este orden: `bassboost` (Bass Boost), `nightcore` (Nightcore), `vaporwave` (Vaporwave), `3d` (8D), `karaoke` (Karaoke), `echo` (Echo), `surround` (Surround) y `tremolo` (Tremolo).
2. Argumentos de ffmpeg. Los define DisTube 5.2.3, no `src/`:
   - `3d`: `apulsator=hz=0.125`
   - `bassboost`: `bass=g=10`
   - `echo`: `aecho=0.8:0.9:1000:0.3`
   - `karaoke`: `stereotools=mlev=0.1`
   - `nightcore`: `asetrate=48000*1.25,aresample=48000,bass=g=5`
   - `vaporwave`: `asetrate=48000*0.8,aresample=48000,atempo=1.1`
   - `surround`: `surround`
   - `tremolo`: `tremolo`
3. Solo un filtro a la vez. Fijar otro reemplaza al anterior.
4. Fijar el mismo filtro otra vez no hace nada: no reinicia la pista.
5. `none` o un nombre vacío quita el filtro.
6. Un nombre que no está en el catálogo: `ok: false`, sin cambios.
7. Los mensajes llevan la etiqueta del filtro.
8. Opciones de entrada de ffmpeg para flujos por red: `reconnect 1`, `reconnect_streamed 1` y `reconnect_delay_max 5`. Se mantienen.
9. Ejecutable de ffmpeg: `FFMPEG_PATH` si está definido. Si no, `bin/ffmpeg.exe` (S-32). Si no, `ffmpeg` del PATH. Node busca en su lugar `node_modules/ffmpeg-static`.
10. La reproducción abre una tubería perezosa `yt-dlp → ffmpeg → WAV PCM s16le` y Songbird detecta el WAV desde el stream. `ProcesoAudio` mantiene ambos procesos, los mata y los espera al liberar la pista. FFmpeg descarta los frames anteriores para cumplir un seek sobre el pipe no seekable. stderr se descarta para evitar bloquear la tubería; falta mejorar diagnóstico de fallos del proceso.
11. REST, `/filter` y el selector del panel comparten `player::reproduccion::fijar_filtro_con_driver`: reinstalan la pista desde su posición actual al cambiar el filtro, y no reinician si se repite. Seek REST/Discord comparte `buscar_con_driver`; la recuperación de corte inicia el pipeline en la posición calculada.

## Tests

- Unitarios de `test/actions.test.js`: fijar un filtro, ignorar volver a fijar el mismo, y quitar.
- Propios de Rust: cada filtro da los argumentos del punto 2; un nombre desconocido falla; `none` quita el filtro; el orden del catálogo coincide; el WAV PCM del nuevo pipeline conserva los hashes PCM de Node.
- Paridad con Node: `scripts/filtros-paridad-ref.js` guarda los argumentos de DisTube para cada filtro. El test de Rust compara.
- Prueba de sonido en F4: la salida de ffmpeg con cada filtro, sobre el mismo fragmento de audio, se compara por hash con la de Node.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Los 8 filtros dan los mismos argumentos que DisTube 5.2.3.
- La salida de ffmpeg con cada filtro, sobre el mismo fragmento, coincide byte a byte con la de Node.
- Fijar el mismo filtro dos veces no reinicia la pista.
- `cargo test` pasa sin ffmpeg instalado, usando un proceso falso.
- Ningún archivo pasa de 250 líneas.

## Decisiones

- Los argumentos de ffmpeg de los 8 filtros se copian al repositorio de Rust. No dependen de DisTube.
- Al cambiar de filtro, la pista sigue desde el segundo actual. Si no se puede, empieza desde el principio.
- ffmpeg lo deja en `bin/` el instalador de S-32. S-33 son los tests de paridad.

## Preguntas abiertas

- Ninguna en esta spec.
