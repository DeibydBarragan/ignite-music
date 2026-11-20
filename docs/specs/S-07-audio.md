# Spec S-07: Audio PCM

- Estado: implementado
- Fase: F2
- Módulo: M2
- Origen en Node (solo lectura): `src/voice/audio-transcoder.js` (`downsample48kStereoTo16kMono`, `pcmRms`, `pcmToWav`, `slicePcmWav`)

## Objetivo

Portar a Rust las funciones de PCM de `audio-transcoder.js`, con la misma salida byte a byte.

No se porta `opusStreamToWav`. Songbird decodifica Opus, así que esta spec empieza en el PCM ya decodificado.

## Decisiones

- No se mejora la decimación ni se añade filtro antialias: cualquier mejora rompe la paridad byte a byte con Node. Si algún día se quiere, se decide aparte.
- La paridad se mide sobre PCM ya decodificado. Si el decode de Opus de songbird (crate `opus2`, libopus) difiere del de prism-media, el fallo se mide con voz real en F2 y no invalida esta spec (ver Preguntas abiertas).

## Archivos

- Crear: `rust/crates/voice/src/audio.rs`
- Crear: `rust/crates/voice/tests/audio_parity.rs`
- Crear: `rust/crates/voice/tests/data/` (referencia generada con Node)
- Crear: `scripts/voice-parity-ref.js` (genera la referencia; debe pasar `npm run lint`)
- No tocar: `src/` ni `test/` de Node

## Contrato

Node guarda dos bytes por muestra, en little endian. Rust recibe muestras `i16`. Un frame estéreo son 2 muestras.

- `downsample_48k_stereo_to_16k_mono(pcm: &[i16]) -> Vec<i16>`. Entrada: L y R intercalados a 48 kHz.
- `pcm_rms(pcm: &[i16]) -> f64`
- `pcm_to_wav(pcm: &[i16], sample_rate: u32, channels: u16) -> Vec<u8>`
- `slice_pcm(pcm: &[i16], start_ms: u32, pre_roll_ms: u32) -> &[i16]`

Formato de entrada, de songbird 0.6.0 (commit `3f77b5f`):

- `decoded_voice` es `Option<Vec<i16>>`. Es `None` si el modo no decodifica.
- Estéreo intercalado (L, R, L, R...) a 48 kHz. Es el valor por defecto de `DecodeConfig`.
- Un tick de 20 ms trae 960 frames por canal, es decir 1.920 valores.
- Un paquete perdido trae audio de relleno con la misma forma.
- Si el emisor manda tramas más largas, el tick también es más largo. Ver `src/driver/tasks/udp_rx/ssrc_state.rs`.

## Comportamiento

1. Downsample: cada grupo de 3 frames estéreo da una muestra mono. La muestra sale solo del primer frame del grupo: el promedio de su L y su R. Los otros dos frames no se leen. No hay filtro antialias. Así lo hace Node. Entradas de menos de 12 bytes (6 muestras) dan salida vacía, y las muestras sobrantes al final se ignoran.
2. Redondeo: igual que `Math.round` de JS, es decir `floor((L + R + 1) / 2)`, con división hacia menos infinito (`div_euclid`). No usar `f64::round`. Con L + R = -3, JS da -1 y `f64::round` da -2.
3. RMS: raíz cuadrada de la media de los cuadrados, en `f64`. Se suma en el mismo orden que Node. Entrada vacía da 0.
4. WAV: cabecera de 44 bytes. Campos: `RIFF`, tamaño 36 + datos, `WAVE`, `fmt ` con tamaño 16, formato PCM (1), canales, tasa, tasa de bytes (tasa × canales × 2), alineación de bloque (canales × 2), 16 bits, `data` y tamaño de datos. Igual que `pcmToWav`.
5. Recorte:
   a. Entrada vacía: devuelve vacío. En Node, `slicePcmWav` además envuelve el vacío en una cabecera WAV de 44 bytes; aquí lo hace quien llama con `pcm_to_wav`.
   b. `start_ms` = 0: devuelve todo.
   c. Inicio efectivo = máximo de 0 y (`start_ms` − `pre_roll_ms`). Desplazamiento en bytes = inicio × 32, es decir 16 000 muestras por segundo × 2 bytes, dividido entre 1 000.
   d. Si el desplazamiento es mayor o igual que el largo − 12 800 bytes (400 ms), devuelve todo.
   e. Si no, devuelve la vista desde el desplazamiento. No copia.

## Tests

- Unitarios: redondeo con sumas -3, -1, 1 y 3. RMS de vacío y de silencio. Cabecera WAV byte a byte. Recorte con inicio 0, con audio menor a 400 ms e inicio válido.
- Paridad con Node. `scripts/voice-parity-ref.js` guarda el SHA-256 de cada salida para:
  - los 11 WAV de `test/fixtures/` (16 kHz mono), con recorte en `start_ms` de 0, 500, 1 000 y 1 500, y RMS;
  - 1 000 entradas sintéticas de 48 kHz estéreo, con semilla fija, para downsample. Los WAV de fixtures no sirven para esto porque están a 16 kHz mono.
- Pruebas de Node que ya cubren esto: `test/voice.test.js` (downsample, `slicePcmWav`) y `test/wake-gate.test.js` (`pcmRms`).

## Criterio de aceptación

- Para cada caso de paridad, el SHA-256 de Rust es igual al de Node.
- `pcm_rms` da el mismo `f64` que Node, con igualdad exacta.
- `slice_pcm` devuelve una vista sobre la misma memoria.
- Ningún archivo pasa de 250 líneas.
- `npm run lint` pasa con el script nuevo.

## Preguntas resueltas

- Formato de `decoded_voice`: ver Contrato. Fuente: `src/events/context/data/voice.rs` y `src/driver/decode_mode.rs` del commit `3f77b5f`.
- Todas las reglas de Comportamiento verificadas contra `src/voice/audio-transcoder.js` actual. Confirmado: los 11 WAV de `test/fixtures/`, `test/voice.test.js` y `test/wake-gate.test.js` existen, y `npm run lint` cubre `scripts/`.

## Preguntas abiertas

- Si el decode de Opus de songbird (crate `opus2`, libopus) difiere del de prism-media, el PCM de entrada no coincide con Node. Eso se mide con voz real en F2.
