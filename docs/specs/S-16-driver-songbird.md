# Spec S-16: Driver de songbird

- Estado: implementado
- Fase: F2 (solo lo que F2 necesita; el resto va en F4)
- Módulo: M4
- Fuente leída: songbird v0.6.0, commit `3f77b5f`.
- Origen en Node (solo lectura): `src/music/playback.js` y `src/api/v1/ops/voice.js` (unión con `distube.voices.join`). Hoy DisTube reproduce. Esa parte no se porta aquí.

## Objetivo

Unir el bot a un canal de voz, reproducir un WAV local, pausar, reanudar y salir. Con songbird 0.6.0, twilight 0.17 y un solo shard.

Fuera de esta spec: yt-dlp (S-13), cola (S-17), saltar y filtros (S-17 y S-18).

## Decisiones

- TLS con rustls, no con Schannel. Verificado en `twilight-gateway` 0.17.1: los features por defecto ya traen `rustls-platform-verifier` (usa el almacén de certificados de Windows), y el feature `native-tls` activa `tokio-websockets/openssl`, que pide compilar OpenSSL (Perl y NASM en Windows). Con rustls todo es Rust puro y no se añade herramienta; la máquina solo tiene VS Build Tools y CMake (S-31).
- Plan B: `native` en songbird y, en twilight-gateway, `default-features = false` con `features = ["native-tls", "zstd", "twilight-http"]`, asumiendo el build de OpenSSL.

## Archivos

- Crear: `rust/crates/player/src/driver.rs`
- Crear: `rust/crates/player/tests/driver_manual.rs` (prueba con `#[ignore]`; necesita token y servidor de prueba)
- Modificar: `rust/crates/player/Cargo.toml` (dependencias, ver Dependencias)
- No tocar: Node

## Contrato

- `join(guild, canal) -> Result<(), DriverError>`
- `play_file(guild, ruta) -> Result<TrackHandle, DriverError>`
- `pause(guild)`, `resume(guild)`, `stop(guild)`
- `leave(guild)`

## Comportamiento

Verificado en el código del commit `3f77b5f` (`examples/twilight`, `examples/serenity/voice_receive`, `src/manager.rs`, `src/shards.rs`, `src/config.rs` y `src/driver/decode_mode.rs`) y de nuevo contra el crate publicado 0.6.0 (ver Preguntas resueltas).

1. Se crea con `Songbird::twilight_from_config(Arc::new(TwilightMap::new(senders)), user_id, config)`. `senders` es un `HashMap<u32, MessageSender>` con `shard.id().get() as u32` y `shard.sender()` de cada shard. `MessageSender` se reexporta en la raíz de `twilight_gateway`. `user_id` es el id del bot. Songbird no usa caché.
2. `config` es `Config::default().decode_mode(DecodeMode::Decode(DecodeConfig::default()))`. Con `Songbird::twilight` sin config, el modo es `DecodeMode::Decrypt`, que no entrega PCM.
3. Songbird 0.6.0 declara soporte para twilight `>= 0.16.0, <= 0.17`. Se usa twilight 0.17.
4. El gateway necesita el intent `GUILD_VOICE_STATES`.
5. Cada evento del gateway pasa por `songbird.process(&event).await`. Sin eso, songbird no recibe `VoiceStateUpdate` ni `VoiceServerUpdate`.
6. Los manejadores de voz se añaden antes de `join`. Primero, `manager.get_or_insert(guild)` crea el `Call`. Después se llama `add_global_event` por cada evento. Algunos eventos de voz llegan durante el join.
7. `join(guild, canal)` devuelve `JoinResult<Arc<Mutex<Call>>>`. Si falla, el `Call` queda creado, así que se limpia con `leave(guild)` (ver 8).
8. `leave(guild)` llama a `Songbird::leave(guild)`. Sale del canal y limpia el `Call` (`connection = None`, `driver.leave()`). No existe `Songbird::remove` en 0.6.0. Tras un join fallido, `leave` también limpia sin error: `Call::leave` solo manda el update de voz si había canal activo.
9. `TrackHandle`: `pause()` pausa, `play()` reanuda y `stop()` para.
10. Archivos locales: `songbird::input::File::new(ruta).into()` y luego `call.play_input(...)`. No usa ffmpeg. Songbird no trae códecs de Symphonia por defecto, así que se añade Symphonia (ver Dependencias).

## Dependencias

```toml
songbird = { version = "=0.6.0", default-features = false, features = ["driver", "gateway", "receive", "tungstenite", "rustls", "twilight"] }
symphonia = { version = "0.5.2", default-features = false, features = ["wav", "pcm"] }
twilight-gateway = "0.17"
twilight-http = "0.17"
twilight-model = "0.17"
```

- `default-features = false` quita `serenity`, que no se usa.
- `rustls` es el TLS del driver (`tokio-tungstenite/rustls-tls-webpki-roots` y `reqwest/rustls-tls`). El gateway usa su propio TLS por defecto (`rustls-platform-verifier`). No se usa `native` (ver Decisiones).
- `tungstenite` es el websocket de voz y también el de por defecto. `ws.rs` tiene una rama por cada websocket.
- Symphonia debe ser 0.5.2, la línea que usa songbird 0.6.0. La rama `main` ya es 0.6.1 y no comparte crate con songbird.
- DAVE no tiene feature propia. Viene con `driver`, mediante el crate `davey` 0.1.2.

## Tests

- Manual, con `#[ignore]`: unir, reproducir un WAV de prueba, pausar 3 s, reanudar, parar y salir.
- `cargo test` pasa sin token, porque la prueba manual está ignorada.
- Gates: `cargo tree -i openssl` y `cargo tree -i native-tls` no muestran nada. Confirma que el árbol TLS es rustls puro (ver Decisiones).

## Criterio de aceptación

- En el servidor de prueba, unir, reproducir, pausar, reanudar, parar y salir funcionan en ese orden, y el audio coincide con el estado.
- Un canal que no existe devuelve `DriverError`. Después, el bot puede volver a unirse.
- Mientras reproduce, el bot sigue oyendo al usuario (S-08).

## Preguntas resueltas

- `DecodeMode` con twilight: ver Comportamiento 2. Por defecto es `Decrypt` (`Config::default`).
- Entrada de archivos locales: ver Comportamiento 10 y Dependencias.
- Shard y caché: songbird no usa caché. Quién crea los shards lo decide S-21. La prueba manual crea los suyos igual que el ejemplo.
- Verificado contra el crate publicado songbird 0.6.0 y twilight 0.17.1 (2026-11-08):
  - `twilight_from_config` recibe `Arc<TwilightMap>`; `TwilightMap::new` recibe `HashMap<u32, MessageSender>`.
  - El rango de twilight de songbird es `>= 0.16.0, <= 0.17`. El `Cargo.lock` publicado de songbird fija 0.16.0, pero el código compila con 0.17: `process` accede a `VoiceStateUpdate` como tupla (`v.0`), forma que mantiene 0.17.1. `MessageSender` está en la raíz de `twilight_gateway` y `Shard::id()` devuelve `ShardId` (de ahí `Id::get()`).
  - `Call` implementa `Deref`/`DerefMut` a `Driver`, así que `add_global_event` se llama directo sobre el `Call`.
  - La feature `receive` activa `discortp/demux` y `discortp/rtcp`.
  - `davey` 0.1.2 viene con `driver`.

## Preguntas abiertas

- Si `join` falla solo con DAVE, se aplica la regla de parada del plan. El código de recepción sí descifra DAVE (`davey` 0.1.2), pero eso no prueba voz real. Se prueba en F2.
