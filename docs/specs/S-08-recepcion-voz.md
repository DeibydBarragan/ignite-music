# Spec S-08: Recepción de voz

- Estado: integrado en el driver. Aceptación con voz real en DAVE pendiente
- Fase: F2
- Módulo: M2
- Origen en Node (solo lectura): `src/voice/receiver.js`, `src/voice/speaker-filter.js`, `src/voice/admission.js`. Valores por defecto en `src/config.js`.

## Objetivo

Convertir la voz que recibe songbird en fragmentos por usuario. Cada fragmento pasa por admisión. Si entra, se recorta y se entrega a transcripción (S-09).

Fuera de esta spec: la compuerta de wake word (S-06), Groq (S-09), el matcher (S-10) y el registro de eventos (S-12). Aquí se prueban con implementaciones de prueba.

## Decisiones

- Se reutiliza el dato del miembro cuando Discord lo incluye en el estado de voz. Para hablantes desconocidos se consulta `twilight-http` con `client.guild_member(guild_id, user_id)`. Si falla la identificación, se descarta el audio hasta poder identificar al hablante. Node también combina caché con una consulta de miembro. La implementación no pide el intent privilegiado `GUILD_MEMBERS`.

## Archivos

- Crear: `rust/crates/voice/src/receive.rs` (segmentación por usuario)
- Crear: `rust/crates/voice/src/speakers.rs` (filtro de bots)
- Crear: `rust/crates/voice/src/admission.rs` (volumen, compuerta y cooldown)
- Crear: `rust/crates/voice/tests/receive.rs`
- No tocar: Node. Lee las mismas variables de `.env` que hoy.

## Contrato

Entrada, de songbird 0.6.0 (commit `3f77b5f`). Verificado en `examples/serenity/voice_receive` y en `src/events/`:

- `CoreEvent::SpeakingStateUpdate`: asocia SSRC con `user_id`. El `user_id` es opcional.
- `CoreEvent::VoiceTick`: cada 20 ms. `speaking` tiene un `VoiceData` por SSRC con voz o con relleno de pérdida. `silent` tiene los SSRC conocidos sin audio en ese tick.
- `CoreEvent::ClientDisconnect`: `user_id` que sale del canal.
- Requiere la feature `receive` de songbird y `DecodeMode::Decode` activo. Por defecto songbird usa `Decrypt`.
- El tick llega unos 100 ms después del paquete: `playout_buffer_length` (5) × 20 ms.

Salida: fragmento por usuario, en 48 kHz estéreo, con `guild_id`, `user_id` y duración de voz en ms. Admisión lo baja a 16 kHz mono con S-07, como hace Node.

## Comportamiento

1. Un fragmento activo por usuario. Voz nueva de ese usuario se añade al fragmento activo.
2. Un fragmento empieza cuando un usuario aparece en `speaking` y no tiene fragmento activo.
3. Un fragmento termina cuando el usuario pasa `VOICE_SILENCE_DURATION_MS` sin voz. Por defecto 500 ms, rango de 400 a 3 000. El silencio se cuenta en ticks de 20 ms. Un tick sin voz es uno en que el SSRC no aparece en `speaking` (aparece en `silent`). Un hueco de pérdida dentro de la voz sí aparece en `speaking` y no corta el fragmento.
4. Se descartan fragmentos de menos de 650 ms de voz.
5. A los 12 000 ms de voz, el fragmento se corta y se procesa.
6. `ClientDisconnect` cierra el fragmento activo y aplica la regla 4.
7. Se ignora el audio del propio bot (el `user_id` se conoce desde S-16). Se ignora a otros bots, salvo el bot de sonido configurado con `soundBotId` y `allowBotInput` de `config.voice`. El dato de bot se consulta con `twilight-http` (ver Decisiones).
8. Admisión, en este orden:
   a. Volumen: RMS en 16 kHz mono de al menos `VOICE_MIN_RMS` (200 por defecto). Si no, se descarta sin registro.
   b. Compuerta: se llama a S-06 con las wake words del servidor. Si cierra, se registra `filtered_out` en S-12 y termina.
   c. Cooldown: si el mismo `guild:usuario` pidió a Groq hace menos de `VOICE_GROQ_COOLDOWN_MS` (2 000 por defecto), se registra `rate_limited` y termina.
   d. Admitido: se guarda el instante para el cooldown. Se recorta con `slice_pcm` (S-07) desde el inicio de la compuerta, con 250 ms de pre-roll. Se entrega a S-09.

## Tests

- Segmentación, con secuencias sintéticas de ticks:
  - 1 000 ms de voz y 600 ms de silencio: un fragmento de 1 000 ms.
  - 300 ms de voz: se descarta.
  - 13 000 ms de voz: corte a los 12 000 ms.
  - Dos usuarios a la vez: dos fragmentos independientes.
  - Un bot en el canal: ningún fragmento.
  - `ClientDisconnect` a mitad de voz: cierra el fragmento.
- Admisión, con compuerta de prueba (abierta o cerrada) y reloj controlado:
  - RMS bajo: se descarta sin registro.
  - Compuerta cerrada: `filtered_out`.
  - Dos peticiones a menos de 2 000 ms: la segunda da `rate_limited`.
- Paridad con Node: `admitAudio` de Node con `offlineGating: false`, que no necesita Vosk. Con los WAV de fixtures (16 kHz mono, como los recibe `admitAudio`) y los mismos instantes, las decisiones de RMS y de cooldown deben coincidir.
- El test del gateway entrega los dos eventos de Discord al adaptador real de Songbird mientras una interacción espera la conexión. Otra interacción progresa antes del segundo evento.
- La prueba manual `cargo test -p bot --test recepcion_manual -- --ignored --nocapture` utiliza `IGNITE_VOICE_TEST_TOKEN`, `IGNITE_VOICE_TEST_GUILD_ID`, `IGNITE_VOICE_TEST_CHANNEL_ID`, `IGNITE_VOICE_TEST_SPEAKER_ID` y `IGNITE_VOICE_TEST_EXPECTED_MS` (diez duraciones de referencia separadas por comas). Exige un canal de prueba con DAVE y un locutor humano. Comprueba muestras PCM con señal, hablante, duración y ausencia de fragmentos adicionales mientras el WAV se reproduce. Repite el ciclo después de salir y volver a entrar.

## Criterio de aceptación

- En el servidor de prueba, con voz real en un canal con DAVE, 10 frases dichas por una persona dan 10 fragmentos. Cada duración de voz queda a ±100 ms de lo dicho. El ±100 ms es un valor propuesto.
- Un bot que reproduce música en el canal no produce fragmentos.
- Pasan los tests de segmentación, admisión y paridad.
- Ningún archivo pasa de 250 líneas.

## Preguntas resueltas

- Duración de un tick: 20 ms (`AUDIO_FRAME_RATE` = 50, en `src/constants.rs`).
- Formato de `decoded_voice`: ver S-07.
- Voz de un SSRC sin usuario: con DAVE, los paquetes cifrados de un SSRC sin usuario se descartan antes de llegar al decodificador (`src/driver/tasks/udp_rx/mod.rs`). Sin DAVE, el paquete llega a `speaking` solo con el SSRC. S-08 lo descarta porque no tiene `user_id`.
- Fuente del dato de bot: ver Decisiones. Verificado que `twilight-http` 0.17.1 tiene `Client::guild_member(guild_id, user_id)` (GET member), igual que el `guild.members.fetch(userId)` de Node.
- Los payloads de `SpeakingStateUpdate` y `ClientDisconnect` vienen de `serenity-voice-model` 0.3.0, reexportado como `songbird::model`. `Speaking` trae `ssrc: u32` y `user_id: Option<UserId>`; `ClientDisconnect` trae `user_id: UserId`.
- Verificado en el crate publicado songbird 0.6.0: `VoiceTick { speaking: HashMap<u32, VoiceData>, silent: HashSet<u32> }`, `VoiceData.decoded_voice: Option<Vec<i16>>`, y `playout_buffer_length` por defecto 5 (100 ms). La feature `receive` activa `discortp/demux` y `discortp/rtcp`.

## Preguntas abiertas

- Si DAVE impide recibir voz, la spec falla y se aplica la regla de parada del plan. El código sí descifra DAVE. Falta la prueba con voz real en Tests.
