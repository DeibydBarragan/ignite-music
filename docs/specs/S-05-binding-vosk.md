# Spec S-05: Binding de Vosk

- Estado: implementado
- Fase: F3
- Módulo: M2
- Origen en Node (solo lectura): `src/voice/vosk-binding.js` (`initVoskApi`, `VoskWrapper`), `src/voice/model-manager.js` (`VoskModelManager`: `initialize`, `acquireRecognizer`, `releaseRecognizer`, `processUtterance`, `destroy`), `test/vosk.test.js` (grupos VoskModelManager y pool).

## Objetivo

Cargar `libvosk` desde Rust, cargar el modelo acústico y dar reconocedores para audio de 16 kHz. Lo usan S-06 (puerta de palabra de activación) y S-09 (respaldo local de transcripción).

No se porta aquí la descarga de la DLL ni de los modelos (S-32). Esta spec solo recibe rutas.

## Archivos

- Crear: `rust/crates/voice/src/vosk.rs` (FFI con `libloading`)
- Crear: `rust/crates/voice/src/vosk_modelo.rs` (modelo y pool de reconocedores)
- Crear: `rust/crates/voice/tests/vosk.rs`
- Crear: `rust/crates/voice/tests/data/vosk-texto.json` (texto que devuelve Node para los WAV de prueba)
- Crear: `scripts/vosk-paridad-ref.js` (genera ese archivo; debe pasar `npm run lint`)
- Modificar: `rust/crates/voice/Cargo.toml` (dependencia `libloading`)
- No tocar: `src/voice/vosk-binding.js`, `src/voice/model-manager.js`, `bin/vosk/`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada de carga: ruta a la DLL (`BIN_DIR/vosk/libvosk.dll`) y ruta a la carpeta del modelo.
- Entrada de transcripción: audio PCM de 16 bits, mono, 16 kHz, little endian, sin cabecera WAV.
- Salida: texto en minúsculas y recortado. Cadena vacía si no hay texto.
- Errores:
  - `Biblioteca`: la DLL no carga o falta un símbolo.
  - `Modelo`: `vosk_model_new` devuelve nulo. Mensaje: `Failed to load Vosk model from "<ruta>"`.
  - `Reconocedor`: no se crea el reconocedor. Mensaje: `Failed to create VoskRecognizer`.

## Comportamiento

1. La DLL se carga una vez. Si ya está cargada, se usa la primera ruta. Node hace lo mismo: `initVoskApi` ignora rutas nuevas.
2. Al cargar, se llama `vosk_set_log_level(-1)` para silenciar Kaldi.
3. Node enlaza 13 funciones al cargar. El código de producción de F3 usa 11: `vosk_set_log_level`, `vosk_model_new`, `vosk_model_free`, `vosk_model_find_word`, `vosk_recognizer_new`, `vosk_recognizer_new_grm`, `vosk_recognizer_free`, `vosk_recognizer_accept_waveform`, `vosk_recognizer_final_result`, `vosk_recognizer_reset` y `vosk_recognizer_set_words`. Las otras dos, `vosk_recognizer_result` y `vosk_recognizer_partial_result`, solo las usa código sin uso (ver Decisiones). Rust carga las 11 funciones requeridas por S-05 y S-06.
4. `hasWord(modelo, palabra)`: pasa la palabra a minúsculas y quita espacios. Devuelve verdadero si `vosk_model_find_word` da 0 o más.
5. Reconocedor sin gramática: `vosk_recognizer_new(modelo, 16000)`.
6. Reconocedor con gramática (lista de frases): se serializa a JSON y se usa `vosk_recognizer_new_grm`. S-06 lo usa para la puerta de palabras de activación.
7. `setWords(rec, true)` activa tiempos y confianza por palabra. S-06 lo necesita.
8. `acceptWaveform`: el largo se pasa en bytes. Un buffer vacío devuelve 0 sin llamar a la DLL.
9. Los resultados son JSON. Si no se pueden leer, el resultado es vacío.
10. Carga del modelo: si `vosk_model_new` devuelve nulo, se espera 200 ms y se reintenta una vez. Node hace lo mismo en `VoskModelManager.initialize`.
11. Pool de reconocedores sin gramática, con clave `__full__`:
    a. `adquirir()` saca un reconocedor inactivo o crea uno nuevo.
    b. `liberar(rec)` hace `reset`. Lo guarda si hay menos de 4 inactivos. Si no, lo libera.
    c. Sin modelo listo, `adquirir()` no devuelve nada.
12. `procesar_enunciado(rec, pcm)`: acepta el audio, lee el resultado final, hace `reset` y devuelve el texto en minúsculas y recortado. S-09 lo usa.
13. `destroy()`: libera los reconocedores inactivos y el modelo. Después `listo` es falso.

## Tests

- Unitarios con un backend falso, sin DLL:
  - El pool toma, reutiliza, resetea, guarda hasta 4 inactivos y libera el quinto.
  - Sin modelo, `adquirir()` no devuelve nada.
  - Un buffer vacío devuelve texto vacío.
- Con la DLL real. Si falta `bin/vosk/libvosk.dll`, el test se marca como ignorado y avisa:
  - `ignite-sample.wav` y `casual-speech-sample.wav` dan el texto de `tests/data/vosk-texto.json`.
- Paridad con Node: `scripts/vosk-paridad-ref.js` guarda el texto que da `processUtterance` de Node para cada WAV de `test/fixtures/`, sin la cabecera de 44 bytes. El test de Rust compara.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Con la DLL y el modelo en `bin/vosk` y `data/models`, `procesar_enunciado` da el mismo texto que Node en los WAV de prueba.
- El pool nunca guarda más de 4 reconocedores inactivos.
- Sin la DLL, `cargo test` pasa, con los tests de DLL marcados como ignorados.
- Ningún archivo pasa de 250 líneas.

## Decisiones

- Código sin uso de Vosk: no se porta. Es `processAudioChunk`, `getResult`, `getPartialResult` y `findWakeWordMatch`. `findWakeWordMatch` está en `matcher.js` (ver S-10). Node lo prueba en `test/vosk.test.js`, pero ningún código del bot lo llama. Por eso `vosk_recognizer_result` y `vosk_recognizer_partial_result` quedan fuera del binding, mientras que `vosk_recognizer_new_grm` sí se incluye para la gramática de S-06.

## Preguntas resueltas

- Modelo para la transcripción local: la ruta del modelo llega como parámetro a la carga (decisión del plan, 9.7). Node la elige en `model-downloader.js` (`ensureVoskModel`): en producción usa el español (`vosk-model-small-es`) si la carpeta existe, y si no, el inglés (`vosk-model-small-en-us-0.15`); en entorno de prueba (`NODE_TEST_CONTEXT` o `npm test`) usa siempre el inglés. El arranque de Rust replica esa elección y pasa la ruta a esta spec.
- Símbolos: Rust carga los 11 símbolos que usan S-05 y S-06 (regla 3). Las funciones auxiliares `vosk_recognizer_result` o `vosk_recognizer_partial_result` quedan fuera.
- DLL con runtime de MinGW: resuelto en S-31. Node.exe está compilado con MSVC y hoy carga `libvosk.dll` (MinGW) con sus DLL de runtime al lado en `bin/vosk/`. Un proceso Rust con target MSVC hace lo mismo. El test con la DLL real lo confirma en Rust.
