# Spec S-10: Matcher y despacho de comandos de voz

- Estado: borrador
- Fase: F3
- Módulo: M2 (el despacho usa la música de M4 y el panel de M5)
- Origen en Node (solo lectura): `src/voice/matcher.js` (`matchVoiceCommand` y funciones auxiliares; `DEFAULT_COMMAND_BINDINGS`, `COURTESY_PREFIXES`, `ALLOWED_NON_ARG_SUFFIXES`, `WAKE_WORD_PHONETIC_VARIANTS`), `src/voice/dispatcher.js` (`dispatchVoiceCommand`), `src/music/events.js` (conexión con el receptor). Tests: `test/voice.test.js` (matcher y despacho), `test/vosk.test.js` (matcher).

## Objetivo

Convertir el texto transcrito en un comando (`commandId` y consulta). Después ejecutar el comando sobre la música y avisar con TTS (S-11).

No incluye la transcripción (S-09), la puerta (S-06), la música (S-17 y S-18) ni los ajustes guardados del servidor (S-30).

## Archivos

- Crear: `rust/crates/voice/src/matcher.rs` (normalización, palabras de activación y frases)
- Crear: `rust/crates/voice/src/comandos.rs` (tabla de frases por defecto)
- Crear: `rust/crates/voice/src/despacho.rs` (ejecución contra la música)
- Crear: `rust/crates/voice/tests/matcher.rs` (casos de paridad)
- Crear: `rust/crates/voice/tests/despacho.rs` (música falsa)
- Crear: `rust/crates/voice/tests/data/matcher-casos.json` (generado con Node)
- Crear: `scripts/matcher-paridad-ref.js` (debe pasar `npm run lint`)
- No tocar: `src/voice/matcher.js`, `src/voice/dispatcher.js`, `src/music/events.js`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada de `coincidir(texto, opciones)`:
  - Texto transcrito.
  - Palabras de activación: lista, o cadena separada por comas. Por defecto `ignite`.
  - Frases personalizadas del servidor, opcional (sale de S-30).
- Salida de `coincidir`: `Coincidencia { coincide, motivo?, palabra?, comando?, frase?, consulta?, falta_consulta?, relleno? }`.
- Entrada de `despachar`: `Solicitud { servidor, usuario, texto, canal? }`, los ajustes del servidor (S-30) y una música que cumpla el trait `Musica` (S-17 y S-18).
- Salida de `despachar`: `Resultado { ejecutado, comando?, consulta?, motivo?, mensaje_tts? }`.
- Motivos de no ejecución. Son los mismos textos que Node, porque el registro (S-12) los guarda:
  - `empty_text`, `wake_word_missing`, `only_wake_word`, `command_not_found`.
  - `voice_disabled_for_guild`, `missing_argument`, `user_not_in_voice`, `no_active_queue`, `guild_not_found`.
  - `unsupported_command`, `execution_error`.

## Comportamiento

1. Normalización del texto: quita los acentos (también á con diéresis, y las mayúsculas acentuadas), pasa a minúsculas, cambia por espacio los signos `¿ ? ¡ ! . , ; : ( ) [ ] { } " ' \ / ~ _ # * < >` y los guiones, colapsa espacios y recorta. La ñ no cambia.
2. Palabras de activación: si la entrada está vacía, `ignite`. Si es texto, se separa por comas. Cada palabra se normaliza. Si la lista queda vacía, `ignite`.
3. Variantes de la palabra de activación. Para `ignite` estas formas que escribe Whisper: `ignait`, `ignaite`, `inight`, `inayt`, `ignit`, `ignayt`, `inite`, `ignete`. Una palabra con espacio también se busca sin espacio. Una palabra de 8 o más letras se busca partida con un espacio en cualquier punto entre la letra 3 y la letra len-3. Las formas se ordenan de la más larga a la más corta.
4. Apariciones: la palabra de activación se busca como palabra completa, sin distinguir mayúsculas. Cada aparición guarda el texto antes (prefijo) y el texto después (resto).
5. Frases: primero las de `DEFAULT_COMMAND_BINDINGS`, después las del servidor, que se suman. Todas se normalizan y se ordenan de la más larga a la más corta.
6. Por cada aparición, en orden:
   a. Saludo: si el prefijo termina en `hola` u `ola`, y el resto empieza por `que haces`, `ke haces`, `k ase` o `q haces`, el comando es `hola_que_hace`.
   b. Coincidencia directa: la frase debe empezar el resto y seguir un espacio, o ser el resto completo. Si el comando no lleva consulta, lo que sigue a la frase debe estar en la lista de sufijos permitidos (vacío, `por favor`, `porfa`, `favor`, `gracias`, `muchas gracias`, `ya`, `ahora`, `de una`, y las formas de `la cancion`, `el tema`, `la musica`, `la pista`, `la reproduccion`, `la lista`, `la cola`). Si no, esa frase se descarta.
   c. La preposición `para`: se descarta si la primera palabra del resto termina en `ar`, `er`, `ir`, `le`, `les`, `me`, `te`, `se` o `nos`, o es una de `que`, `mi`, `ti`, `el`, `la`, `los`, `las`, `un`, `una`, `eso`, `esto` o `nada`.
   d. Consulta: en `play`, `play_next`, `enqueue` y `volume`, la consulta es el resto después de la frase, sin conectores del principio (`la cancion de`, `el tema de`, `la musica de`, `a`, `de`, `por`). Si queda vacía, `falta_consulta` es verdadero.
   e. Cortesía: si el resto empieza por un prefijo de cortesía (`por favor puedes`, `porfa puedes`, `hazme el favor de`, `hazme el favor y`, `por favor`, `porfa`, `puedes`, `favor de`, `please`, `can you`), se prueba la coincidencia directa en lo que sigue. El prefijo queda en `relleno`.
   f. Gana la primera coincidencia.
7. Sin coincidencia:
   a. Si la última aparición no tiene resto: `only_wake_word`.
   b. Si la última aparición tiene resto: `command_not_found`.
   c. Sin aparición: `wake_word_missing`. Si el texto es el saludo `hola que haces` sin palabra de activación, el comando es `hola_que_hace`.
8. Texto vacío: `empty_text`.
9. Despacho: si el ajuste del servidor tiene `enabled` en falso, `voice_disabled_for_guild`. Si no hay coincidencia, se devuelve el motivo del matcher.
10. `play` sin consulta: si hay cola y está pausada o parada, se reanuda (`resume`) y se avisa. Si no, `missing_argument`.
11. Servidor: se busca en caché o en Discord. Si no existe, `guild_not_found`.
12. Canal de voz: el del miembro, o el de la cola, o el indicado en la solicitud. Sin canal, `user_not_in_voice` para `play`, `play_next` y `enqueue`.
13. Comandos:
    - `play`: aviso TTS, luego `manager.play` con `skip` en verdadero, sin posición.
    - `play_next`: aviso TTS, luego `play` con posición 1 y sin saltar.
    - `enqueue`: aviso TTS, luego `play` con posición 0 y sin saltar.
    - `pause`, `resume`, `skip`, `previous`, `stop`, `shuffle` y `volume`: sin cola, `no_active_queue`. `volume` usa `parseInt` sobre la consulta.
    - `autoplay_on` y `autoplay_off`: fijan el estado de forma idempotente y refrescan el panel.
    - `queue`: devuelve el número de canciones.
    - `hola_que_hace`: devuelve éxito con el mensaje.
    - Cualquier otro: `unsupported_command`.
14. Aviso TTS (S-11): en los comandos que no son `play`, `play_next` ni `enqueue`, el aviso sale después de la acción. En `hola_que_hace` el aviso sale aunque el TTS esté apagado.
15. Un error durante la ejecución se devuelve como `execution_error` con el mensaje. No sale del despacho.

## Tests

- Casos de paridad (`matcher-casos.json`, generado con Node). Muestra:

| Texto | Palabra | Resultado |
| :--- | :--- | :--- |
| `Ignite, reproduce Bohemian Rhapsody!` | ignite | play, consulta `bohemian rhapsody` |
| `oye ignite reproduce starboy` | ignite | play, consulta `starboy` |
| `ignite pon` | ignite | play, `falta_consulta` |
| `ignite` | ignite | `only_wake_word` |
| `ignite un dia de estos pon musica` | ignite | no coincide |
| `ayer en la night pon algo de reggaeton` | ignite | no coincide |
| `ignite siguiente` | ignite | skip |
| `ignait siguiente` | ignite | skip |
| `ignite activa el autoplay` | ignite | autoplay_on |
| `ignite desactiva el autoplay` | ignite | autoplay_off |
| `ignite apaga` | ignite | stop |
| `ignite activa el autoplay de la fiesta` | ignite | no coincide |
| `Ignite, hola que hace` | ignite | hola_que_hace |
| `hola ignite que hace` | ignite | hola_que_hace |
| `Creo que se hace un poco más rápido. Bot, pausa.` | bot | pause |
| `bot por favor puedes reproducir Starboy` | bot | play, consulta `starboy` |
| `bot por favor pon starboy` | bot | play, consulta `starboy` |
| `bot siguiente` | ignite, bot | skip, palabra `bot` |
| `reproduce starboy` | ignite, bot | `wake_word_missing` |
| `bot pausa` | ignite | no coincide |
| `bot un dia de estos pon musica` | bot | no coincide |

- Frase de charla con `para hacerle`: el caso está en `test/vosk.test.js`, línea 205. Debe dar no coincide.
- Frase personalizada `tocate Something About Us` con el enlace `bot`: el caso está en `test/voice.test.js`, línea 171. Debe dar `play` con consulta `something about us`.
- Despacho con música falsa, de `test/voice.test.js`:
  - `ignite pausa` ejecuta `pause`.
  - `ignite reproduce starboy` llama a `play` con consulta `starboy`.
  - `autoplay on` y `autoplay off` fijan el estado de forma idempotente.
- Paridad con Node: `scripts/matcher-paridad-ref.js` guarda el resultado de `matchVoiceCommand` de Node para cada caso de la tabla y de los dos tests. El test de Rust compara `commandId`, consulta, `falta_consulta`, palabra, relleno y motivo.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Todos los casos de `matcher-casos.json` dan el mismo resultado que Node.
- Con música falsa, cada comando del punto 13 llama a la acción correcta.
- `cargo test` pasa sin Discord ni Groq.
- Ningún archivo pasa de 250 líneas.

## Decisiones

- `findWakeWordMatch` de `matcher.js` no se porta. No tiene llamadas (ver S-05).

## Preguntas abiertas

- Frases en español e inglés (`skip`, `pause`, `autoplay on`): ¿se mantienen juntas o se separan por idioma?
- La ñ no se normaliza, así que las frases con ñ no tienen variantes. Propuesta: mantenerlo.
- Las frases personalizadas las guarda S-30. El despacho solo las recibe.
- Frases que se pisan: `apaga` es de `stop`, y `apaga el autoplay` es de `autoplay_off`. Node gana la frase más larga. Propuesta: el mismo criterio.
