# Spec S-06: Puerta de palabra de activación

- Estado: implementado
- Fase: F3
- Módulo: M2
- Origen en Node (solo lectura): `src/voice/wake-gate.js` (`WakeGate`: `initialize`, `buildGrammar`, `tokensFor`, `recognize`, `evaluate`, `destroy`; funciones `splitCompound` y `hasVoicedTail`; constantes `WAKE_DECOYS` y `FILLER_WORDS`), `src/voice/admission.js` (llamada a `evaluate`), `test/wake-gate.test.js`.

## Objetivo

Decidir, en local y antes de cualquier petición a Groq, si un enunciado tiene la palabra de activación seguida de algo. Solo si la puerta se abre, S-09 llama a Groq.

Usa S-05 para Vosk, S-07 para el RMS y S-10 para separar las palabras de activación.

No incluye la lectura del comando (S-10) ni la decisión de llamar a Groq (S-09).

## Archivos

- Crear: `rust/crates/voice/src/wake_gate.rs` (puerta: inicialización, evaluación y veredicto)
- Crear: `rust/crates/voice/src/wake_gate_gramatica.rs` (gramática, palabras compuestas y señuelos)
- Crear: `rust/crates/voice/tests/wake_gate.rs`
- Crear: `scripts/puerta-paridad-ref.js` (genera la referencia; debe pasar `npm run lint`)
- Crear: `rust/crates/voice/tests/data/puerta-casos.json` (generado con Node)
- No tocar: `src/voice/wake-gate.js`, `src/voice/admission.js`, `test/fixtures/` (los tests de Rust leen los WAV de ahí, no los copian).

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada de `evaluar(pcm16k, palabras)`:
  - `pcm16k`: PCM de 16 kHz, mono, 16 bits, sin cabecera (sale de S-07).
  - `palabras`: lista, o cadena separada por comas, con las palabras de activación del servidor.
- Salida: `Veredicto { abierta, motivo, oido, palabra?, inicio_ms?, fin_ms?, no_soportadas? }`.
- Motivos posibles: `ok`, `gate_unavailable`, `no_supported_wake_word`, `no_command_after`, `no_wake_word`.
- Errores: `evaluar` no devuelve error. Si la puerta no está lista, da `gate_unavailable` y la puerta queda cerrada.

## Comportamiento

1. La gramática tiene `[unk]` más las palabras que el modelo conoce. Todo lo demás se convierte en `[unk]`. Así el habla normal no puede dar una palabra de activación.
2. Para cada palabra de activación se prueban los modelos en este orden: inglés, después español.
   a. Si un modelo conoce todas las partes de la palabra, se usa ese modelo.
   b. Si un modelo solo puede separar la palabra en partes conocidas, queda como respaldo.
   c. Si ningún modelo la reconoce, la palabra va a `no_soportadas`.
3. Palabras compuestas: `ignitebot` se parte en dos palabras del diccionario. Se prueban los cortes de derecha a izquierda, y cada mitad debe ser palabra del modelo.
4. Señuelos: para `ignite`, se añaden a la gramática `ignore`, `ignored`, `ignores`, `excite` e `invite`, si el modelo los conoce. Así una pronunciación parecida cae en un señuelo y no en la palabra de activación.
5. Palabras de relleno: `oye` y `hey` se añaden si el modelo las conoce. Permiten decir "oye ignite".
6. Se crea un reconocedor nuevo por modelo y por evaluación. No se reutilizan. Se libera al terminar.
7. Se activan tiempos por palabra (`setWords`).
8. La palabra de activación aparece como una secuencia de palabras reconocidas. Se busca en el resultado final, en orden.
9. Hay algo después de la palabra de activación si se cumple una de estas dos:
   a. Hay otra palabra reconocida después de ella.
   b. Hay voz después: una ventana de 100 ms con RMS de 400 o más, que empieza 100 ms después del final de la palabra.
10. Veredictos:
    a. Una aparición con algo después: `ok`, con `palabra`, `inicio_ms` y `fin_ms`.
    b. Hay aparición, pero sin nada después: `no_command_after`.
    c. No hay aparición: `no_wake_word`.
    d. Ninguna palabra es soportada: `no_supported_wake_word`, con la lista en `no_soportadas`.
    e. La puerta no está lista: `gate_unavailable`, sin tocar el audio.
11. Carga: `initialize` carga la DLL (S-05), el modelo inglés y el español si existe `am/final.mdl`. Si falta la DLL o el inglés, la puerta queda cerrada. Si falla la carga, queda cerrada y se registra el error. Node descarga el español al arrancar si falta. En Rust esa descarga es de S-32.
12. Node llama a `initialize` antes de cada evaluación (`admission.js`). En Rust se hace una vez al arrancar.
13. Con `VOICE_OFFLINE_GATING` en falso, la admisión no llama a la puerta (S-08).
14. Mejora del plan, no paridad: la evaluación corre en `spawn_blocking`, para no frenar el runtime de Tokio. Node es síncrono.

## Tests

- Veredictos de `test/wake-gate.test.js`, con los WAV de `test/fixtures/`:

| Archivo | Esperado | Motivo |
| :--- | :--- | :--- |
| `wake-pos-ignite-pausa.wav` | abierta | `ok` |
| `wake-pos-oye-ignite-reproduce.wav` | abierta | `ok` |
| `wake-pos-ignite-pause-en.wav` | abierta | `ok` |
| `wake-neg-ignores.wav` | cerrada | cualquiera |
| `wake-neg-knight-night.wav` | cerrada | cualquiera |
| `wake-neg-excite.wav` | cerrada | cualquiera |
| `wake-neg-invite.wav` | cerrada | cualquiera |
| `wake-neg-spanish-chat.wav` | cerrada | cualquiera |
| `casual-speech-sample.wav` | cerrada | cualquiera |
| `ignite-sample.wav` | cerrada | `no_command_after` |

- `wake-mb-neg-bot-solo.wav`: el veredicto esperado se fija con el propio Node al generar la referencia (ver Decisiones).
- Sin Vosk inicializado: `gate_unavailable`.
- Palabra `zxqvyw`: `no_supported_wake_word`, con `no_soportadas = ["zxqvyw"]`.
- Gramática de `ignite`: contiene `ignore` y `[unk]`, y no contiene `bot`.
- RMS: silencio da 0, y `ignite-sample.wav` da más de 1000.
- Unitarios de las funciones internas: `splitCompound` y `hasVoicedTail`, con audio sintético de silencio y de voz.
- Paridad con Node: `scripts/puerta-paridad-ref.js` guarda `abierta`, `motivo`, `palabra` y `oido` de cada WAV, para `ignite`, para `bot` y para `ignite bot`. El test de Rust compara.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Los 11 WAV de `test/fixtures/` dan el mismo veredicto y motivo que Node para `ignite`, `bot` e `ignite bot`.
- Sin la DLL, la puerta da `gate_unavailable` y nunca abre.
- Se mide la latencia de cada evaluación en Node y en Rust con los mismos WAV. Esta spec no fija un umbral.
- `cargo test` pasa. Los tests con Vosk se saltan con aviso si falta la DLL.
- Ningún archivo pasa de 250 líneas.

## Decisiones

- Corpus: la paridad de F3 usa los 11 WAV de `test/fixtures/`. Un corpus mayor es tarea aparte. Ningún documento menciona otra cifra.
- `wake-mb-neg-bot-solo.wav`: ningún test de Node lo usa. Entra en la paridad de `bot` e `ignite bot`. Su veredicto esperado es el que da Node al generar `puerta-casos.json`. Por su nombre se espera cerrada; si Node la abriera, se revisa antes de congelar la referencia.

## Preguntas resueltas

- Palabras de activación de varias palabras: resuelta (decisión del plan, 9.7). La paridad añade `bot` e `ignite bot` a los 11 WAV. La gramática ya recibe la frase completa; `tokensFor` parte cada palabra y `buildGrammar` la asigna al primer modelo que la conoce completa, con respaldo de partes (regla 2).
- Un reconocedor nuevo por evaluación: se mantiene. Node lo hace porque `reset` conserva el tiempo de la anterior, lo que desplazaría las marcas de palabra.
