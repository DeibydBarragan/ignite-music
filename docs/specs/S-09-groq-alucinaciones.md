# Spec S-09: Cliente de Groq y filtro de alucinaciones

- Estado: implementado
- Fase: F3
- Módulo: M2
- Origen en Node (solo lectura): `src/voice/groq-client.js` (`transcribeAudio`, `isLikelyHallucination`, `normalizeForHallucination`, `HALLUCINATION_PHRASES`, `GROQ_API_URL`), `src/voice/stt.js` (`transcribeUtterance`), `src/voice/receiver.js` (uso del filtro, líneas 144 a 162), `test/voice.test.js` (grupo groqClient), `test/wake-gate.test.js` (prueba de `isLikelyHallucination`).

## Objetivo

Transcribir el WAV admitido con Groq Whisper. Si Groq falla o no hay clave, usar Vosk local (S-05). Descartar lo que Whisper inventa en silencio o con ruido.

No incluye la admisión ni el registro de eventos (S-08 y S-12), ni el comando (S-10).

## Archivos

- Crear: `rust/crates/voice/src/groq.rs` (cliente HTTP)
- Crear: `rust/crates/voice/src/alucinacion.rs` (filtro)
- Crear: `rust/crates/voice/src/stt.rs` (Groq y respaldo Vosk)
- Crear: `rust/crates/voice/tests/groq.rs` (servidor HTTP falso)
- Crear: `rust/crates/voice/tests/alucinacion.rs`
- Crear: `rust/crates/voice/tests/data/alucinacion-casos.json` (generado con Node)
- Crear: `scripts/alucinacion-paridad-ref.js` (debe pasar `npm run lint`)
- No tocar: `src/voice/groq-client.js`, `src/voice/stt.js`, `src/voice/receiver.js`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada de `transcribir`:
  - WAV completo de 16 kHz mono, con cabecera de 44 bytes.
  - Clave de Groq. Puede ir vacía.
  - Modelo, por defecto `whisper-large-v3-turbo`.
  - Idioma, por defecto `es`.
  - Prompt opcional.
  - Tiempo límite, por defecto 10 000 ms.
- Entrada de `transcribir_enunciado`: lo anterior, más el PCM sin cabecera para el respaldo Vosk.
- Salida: `Transcripcion { texto, ms, segmentos }`. `segmentos` puede venir vacío.
- Errores de Groq:
  - `SinClave`: la clave está vacía. No se hace petición.
  - `Estado(código, cuerpo)`: respuesta no 2xx.
  - `Tiempo(ms)`: se agotó el tiempo límite.
  - `Red(texto)`: error de conexión.
- `transcribir_enunciado` no devuelve error por Groq. Si Groq falla, usa Vosk.

## Comportamiento

1. Clave vacía: error `SinClave`, sin petición.
2. WAV que no sea buffer, o de 44 bytes o menos: texto vacío, sin petición.
3. Petición POST multipart a `https://api.groq.com/openai/v1/audio/transcriptions`, con estas partes:
   - `file`: nombre `command.wav`, tipo `audio/wav`.
   - `model`, `response_format` = `verbose_json`, `temperature` = `0.0`.
   - `language`, si hay idioma. `prompt`, si hay prompt.
4. Cabeceras: `Authorization: Bearer <clave>` y `User-Agent` (ver Decisiones).
5. Tiempo límite de 10 000 ms por defecto. Al agotarse, error `Tiempo` con el texto `Groq STT timed out after <ms>ms`.
6. Respuesta no 2xx: error `Estado` con código y cuerpo. Se registra un aviso con el cuerpo.
7. Respuesta correcta: `texto` recortado. `segmentos` es el array del JSON, o vacío.
8. Antes de enviar se registra información con el modelo. Node además registra los 7 primeros caracteres de la clave y su longitud; Rust no registra ningún dato de la clave (ver Decisiones).
9. `transcribir_enunciado`:
   a. Con clave, llama a Groq. Si no llega `prompt`, usa el de la configuración (`VOICE_PROMPT`). El valor por defecto, igual en `stt.js` y en `config.js`, es `Ignite, mierdabot, bot mierda, reproduce, pon, toca, pausa, salta, siguiente, anterior, agrega, añade, cola, lista, volumen, activa autoplay, desactiva autoplay, autoplay on, autoplay off.`
   b. Si Groq devuelve texto, lo usa.
   c. Si Groq falla, registra un aviso y sigue.
   d. Si no hay texto y Vosk está listo, toma un reconocedor del pool de S-05, transcribe el PCM y lo libera. Los segmentos quedan vacíos.
   e. Devuelve `texto`, `ms` y `segmentos`.
10. Filtro `es_alucinacion(texto, segmentos)`:
    a. Normaliza el texto: minúsculas, quita acentos, cambia por espacio todo lo que no sea a-z, 0-9 o espacio, colapsa espacios y recorta.
    b. Vacío: es alucinación.
    c. Está en la lista de frases (punto 11): es alucinación.
    d. Sin segmentos: no es alucinación.
    e. Si todos los segmentos cumplen `no_speech_prob > 0,6` y `avg_logprob < -1`: es alucinación. Un valor que falte cuenta como 0.
    f. En otro caso, no es alucinación.
11. Lista de frases, ya normalizadas: `gracias`, `muchas gracias`, `gracias por ver el video`, `gracias por ver`, `thank you`, `thanks`, `thanks for watching`, `thank you for watching`, `suscribete`, `suscribete al canal`, `no olvides suscribirte`, `subtitulos por la comunidad de amara org`, `subtitulado por la comunidad de amara org`, `subtitulos realizados por la comunidad de amara org`, `amara org`, `you`, `bye`, `adios`, `hasta la proxima`.
12. S-08 descarta el enunciado si el texto está vacío o si `es_alucinacion` da verdadero. Con texto vacío no se registra ningún evento. Con texto no vacío que es alucinación, el evento se registra con estado `hallucination` (S-12).

## Tests

- Unitarios de `es_alucinacion`, de `test/wake-gate.test.js`:
  - `""` da verdadero.
  - `Gracias.` da verdadero.
  - `¡Suscríbete!` da verdadero.
  - `Subtítulos por la comunidad de Amara.org` da verdadero.
  - `Thank you.` da verdadero.
  - `ignite pausa` da falso.
  - `ignite pon algo` con un segmento silencioso (`no_speech_prob` 0,9 y `avg_logprob` -1,5) da verdadero.
  - Con un segmento real (0,05 y -0,2) da falso.
  - Con los dos segmentos a la vez da falso.
- Cliente con servidor HTTP falso:
  - La petición multipart tiene las partes de la sección Comportamiento.
  - Respuesta 200 da el texto.
  - Respuesta 500 da `Estado`.
  - Retraso da `Tiempo`.
  - Sin clave o con WAV de 44 bytes no hay petición.
- Respaldo: sin clave, `transcribir_enunciado` usa Vosk, con un backend falso de S-05.
- Paridad con Node: `scripts/alucinacion-paridad-ref.js` evalúa `isLikelyHallucination` de Node con una lista fija de textos y segmentos. Incluye acentos, puntuación y los casos anteriores. El test de Rust compara.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Todos los casos de `alucinacion-casos.json` dan el mismo resultado que Node.
- Sin clave, o con Groq caído, la transcripción sale de Vosk. El registro dice el motivo.
- `cargo test` pasa sin red ni clave.
- Ningún archivo pasa de 250 líneas.

## Decisiones

- Clave de Groq en el log: no se registra ningún dato de la clave. Node registra los 7 primeros caracteres y la longitud (`groq-client.js`). No hace falta rotar la clave por eso, porque solo salía un prefijo. Conviene revisar dónde se guardan los logs.
- User-Agent: se mantiene el mismo que Node (`Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36`). El código no explica por qué imita Chrome, pero cambiarlo no aporta nada y añade una variable a la paridad de la petición.
- Lista de frases: se mantienen las 19 fijas. Se amplía solo si los registros de F7 muestran alucinaciones nuevas, como cambio aparte (rompe la paridad del filtro).

## Preguntas resueltas

- User-Agent de navegador: ver Decisiones.
- Lista de frases: ver Decisiones.
