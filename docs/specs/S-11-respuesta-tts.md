# Spec S-11: Respuesta por TTS

- Estado: implementado
- Fase: F3
- Módulo: M2
- Origen en Node (solo lectura): `src/voice/tts-feedback.js` (`DEFAULT_TTS_TEMPLATES`, `interpolateTemplate`, `resolveFeedbackMessage`, `sendTtsFeedback`), `src/voice/dispatcher.js` (llamadas), `src/voice/settings-store.js` (voz, tono y velocidad por defecto), `test/voice.test.js` (grupo ttsFeedback).

## Objetivo

Decir en el canal de voz la confirmación de un comando. El audio lo genera ignite-sound, un servicio externo. El bot solo pide el audio por HTTP.

No incluye el comando (S-10) ni la reproducción del audio, que hace ignite-sound en el canal.

## Archivos

- Crear: `rust/crates/voice/src/tts.rs` (plantillas, interpolación y envío)
- Crear: `rust/crates/voice/tests/tts.rs` (servidor HTTP falso)
- Crear: `rust/crates/voice/tests/data/tts-casos.json` (generado con Node)
- Crear: `scripts/tts-paridad-ref.js` (debe pasar `npm run lint`)
- No tocar: `src/voice/tts-feedback.js`, `src/voice/dispatcher.js`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada de `enviar(aviso, opciones)`:
  - `aviso`: `servidor`, `canal`, `usuario` (opcional), `comando`, `parametros` (opcional, por ejemplo `consulta`).
  - `opciones` (todas opcionales): `activado`, `plantillas`, `url`, `secreto`, `voz`, `velocidad`, `tono`.
- Salida: booleano. Verdadero si ignite-sound contestó con 2xx.
- Errores: ninguno sale de `enviar`. Cualquier fallo se registra como aviso y da falso.

## Comportamiento

1. Plantillas por defecto, en español y sin cambios:

| Comando | Plantilla |
| :--- | :--- |
| play | `Reproduciendo {query}` |
| play_next | `Añadida {query} a continuación` |
| enqueue | `Añadida {query} a la lista` |
| skip | `Saltando canción` |
| pause | `Música pausada` |
| resume | `Reanudando música` |
| stop | `Reproducción detenida` |
| previous | `Canción anterior` |
| shuffle | `Lista en modo aleatorio` |
| autoplay_on | `Autoplay activado` |
| autoplay_off | `Autoplay desactivado` |
| volume | `Volumen ajustado al {query}` |
| queue | `Mostrando la lista` |
| hola_que_hace | `Hola que hace` |

2. Plantillas del servidor: se suman a las de por defecto. Si el servidor define el mismo comando, gana su plantilla.
3. Interpolación: cada `{nombre}` se cambia por el valor de `parametros`. Un marcador es una secuencia de letras, números o guion bajo (`\{(\w+)\}` en Node). Si el valor falta o es nulo, se cambia por texto vacío. El resultado se recorta.
4. Sin plantilla para el comando: no se envía.
5. Activación: si se pasa `activado`, manda ese valor. Si no, `VOICE_TTS_FEEDBACK_ENABLED`. Si está apagado, no se envía.
6. Dirección: `url` si se pasa, o `IGNITE_SOUND_API_URL`. Si está vacía, no se envía.
7. Sin servidor o sin canal, no se envía.
8. Petición: POST a `<url sin barras finales>/api/v1/tts/speak`.
   - Cuerpo JSON: `text`, `guildId`, `channelId`. Además `userId`, `voice`, `rate` y `pitch`, si se pasan.
   - Cabeceras: `Content-Type: application/json`. Si hay secreto, `Authorization: Bearer <secreto>`. Si hay usuario, `X-User-Discord-Id`.
   - Tiempo límite de 5 s.
9. Respuesta no 2xx: aviso con el código y el texto de la respuesta. Devuelve falso.
10. Respuesta 2xx: información de confirmación. Devuelve verdadero.
11. Error de red o tiempo agotado: aviso. Devuelve falso.
12. El despacho (S-10) llama a `enviar` sin esperar el resultado. Un fallo no interrumpe el comando.
13. Para `hola_que_hace`, el despacho fuerza la activación. El aviso sale aunque el TTS esté apagado.
14. Voz por defecto (sale de S-30): `es-MX-DaliaNeural`, velocidad 1,0 y tono 1,0.

## Tests

- Plantillas: cada plantilla por defecto da el mismo texto que Node con sus parámetros (`tts-casos.json`).
- Interpolación: `Reproduciendo {query}` con `Starboy` da `Reproduciendo Starboy`. Un marcador sin valor queda vacío.
- Envío con servidor HTTP falso:
  - Ruta `/api/v1/tts/speak`, cuerpo y cabeceras iguales a Node.
  - Respuesta 200 da verdadero.
  - Respuesta 500 da falso y deja un aviso.
  - Tiempo agotado da falso.
- Apagado o sin URL: no hay petición.
- Paridad con Node: `scripts/tts-paridad-ref.js` guarda el cuerpo y las cabeceras que manda `sendTtsFeedback` de Node contra un servidor falso. El test de Rust compara.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- El cuerpo y las cabeceras de la petición coinciden con Node. Las claves del JSON salen en el mismo orden.
- Un fallo de ignite-sound no interrumpe el comando ni bloquea el despacho.
- `cargo test` pasa sin ignite-sound real.
- Ningún archivo pasa de 250 líneas.

## Decisiones

- Las plantillas están fijas en español. Node no las pasa por i18n (S-03). Se mantienen por paridad (decisión del plan, 9.7).
- Node no espera la respuesta de ignite-sound: el despacho llama a `enviar` sin esperar el resultado (regla 12). Se mantiene (decisión del plan, 9.7).

## Preguntas abiertas

- URL de producción de `IGNITE_SOUND_API_URL`. El código tiene como valor por defecto `https://wsnbbnbdc7.execute-api.us-east-2.amazonaws.com/sound` (`config.voice.soundApiUrl`), pero `.env.example` apunta a `http://localhost:3002`. La confirma el usuario (plan, 9.7). Mientras tanto, Rust usa el mismo valor por defecto que Node.
