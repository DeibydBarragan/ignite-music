# Spec S-21: Cliente twilight (un shard)

- Estado: implementado
- Fase: F5
- Módulo: M5
- Origen en Node (solo lectura): `src/bot.js` (`createClient`, `start`, `shutdown`, eventos `ClientReady`, `InteractionCreate` y `VoiceStateUpdate`), `src/setup/launcher.js` (`main`, `stop`, `buildShardEnv`, `ShardingManager`).

## Objetivo

Arrancar un único proceso con un cliente de twilight 0.17 y un único shard. Conectar al gateway, repartir los eventos a los módulos que los usan y apagar en orden. Sustituye al `ShardingManager` de `launcher.js` y a `createClient`, `start` y `shutdown` de `bot.js`.

No incluye los comandos (S-22), los permisos (S-23), el panel (S-24), las vistas (S-25), la API (S-26) ni el túnel (S-29). Esta spec solo los llama en orden.

Reiniciar el proceso corta la música, como hoy. Al arrancar, S-19 restaura las sesiones.

## Archivos

- Modificado: `rust/crates/bot/src/main.rs`: arranque, orden de pasos y señales de cierre.
- Creado: `rust/crates/bot/src/cliente.rs`: cliente HTTP, shard, intents, presencia y despacho de eventos.
- Creado: `rust/crates/bot/src/apagado.rs`: orden del apagado y tiempo límite.
- Modificado: `rust/crates/bot/Cargo.toml`: `twilight-gateway`, `twilight-http`, `twilight-model` y `twilight-cache-inmemory` (feature `permission-calculator`), todas en 0.17, `tokio`, y dependencias de `nucleo`, `player`, `medios` y `voice`.
- Creado: `rust/crates/bot/tests/cliente.rs`: 7 tests unitarios y de paridad.
- No tocar: `src/bot.js`, `src/setup/launcher.js`, el resto de `src/` (incluidos los cambios sin commit del usuario), `rust/crates/voice/` y `rust/crates/player/`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada: variables ya validadas por S-01: `DISCORD_TOKEN`, `CLIENT_ID`, `GUILD_ID` (opcional) y `STATUS` (por defecto `/play`).
- Config: no hay total de shards ni lista de shards. `SHARDS`, `SHARDING_MANAGER` y `SHARDING_MANAGER_MODE` no se leen.
- Intents: `GUILDS | GUILD_VOICE_STATES` (valor 129, igual que en Node). Ningún intent privilegiado.
- Presencia de arranque: actividad de tipo Listening (2) con el texto de `STATUS`, enviada al identificarse.
- Eventos del gateway: solo los que consumen la caché (S-23), las interacciones (S-22) y la voz (S-16, S-20).
- Salida: código 0 tras un apagado completo. Código 1 si falla el arranque o cae una tarea crítica.
- Errores:
  - Configuración inválida: error de arranque (S-01), código 1.
  - Login rechazado (token inválido o intents no permitidos): se registra, código 1.
  - Tarea crítica (gateway o API) que termina con error o pánico: apagado y código 1.
  - Fallo de una tarea no crítica: se registra y el proceso sigue.

## Comportamiento

1. Arranque, en este orden. Es el mismo orden que `launcher.js`:
   1. Logs (S-02) y configuración con preflight (S-01), con los binarios de S-32.
   2. Limpieza de `audio_cache/`, la carpeta que usaba la v16. Node la borra al arrancar.
   3. Registro de comandos (S-22). Si falla, se registra y sigue.
   4. Túnel de Cloudflare (S-29), si está activado.
   5. Versión de yt-dlp (S-13). Si falla, se registra el error y sigue.
   6. Cliente, shard y conexión al gateway.
2. Primer `Ready`: se registra "Conectado como <usuario> · N servidores". Después se cargan los IDs de comandos (S-22), se restauran las sesiones (S-19) y se arrancan la API (S-26) y el anunciador (S-29).
3. Un `Ready` posterior, tras una reidentificación, no repite el paso 2.
4. Un solo shard: `ShardId::ONE` de un total de 1. No hay `broadcastEval`, ni `shard-proxy`, ni variables de entorno para shards hijos.
5. Presencia: se envía en la identificación, con tipo Listening y el texto de `STATUS`.
6. Despacho de eventos:
   1. `InteractionCreate` sin `guild_id` se ignora. En Node, `inCachedGuild()` hace lo mismo con los mensajes directos. Con `guild_id`, la interacción pasa al router de S-22.
   2. `VoiceStateUpdate` pasa a songbird (S-16) y actualiza la caché de voz (S-23). Después avisa a la presencia (S-20).
   3. `VoiceServerUpdate` pasa a songbird (S-16).
   4. Un error al procesar un evento se registra y el bucle sigue.
7. Corte de red: se espera que twilight reconecte el gateway y reanude la sesión sin reiniciar el proceso. Se comprueba a mano en el servidor de prueba.
8. Señales: Ctrl+C inicia el apagado. Las demás señales se fijan en Decisiones.
9. Apagado, en este orden:
   1. Dejar de despachar eventos nuevos.
   2. Parar el anunciador (S-29), cerrar el túnel (S-29) y cerrar la API (S-26), cortando las conexiones abiertas.
   3. Apagar el reproductor (S-17): guardar las colas (S-19), parar los temporizadores (S-20) y salir de los canales.
   4. Volcar los registros de voz en CSV (S-12).
   5. Cerrar el gateway.
   6. Salir con código 0.
   El apagado completo tiene un límite de 10 s, la constante `SHUTDOWN_GRACE_MS` de `launcher.js`. Si se pasa, se registra un aviso y se sale con código 0.
10. Caída de una tarea crítica: se ejecuta el apagado del paso 9 y se sale con código 1. Node hace lo mismo ante una excepción no controlada.
11. Fallo de una tarea no crítica: se registra. Node registra las promesas no manejadas y no sale.

## Tests

- Unitarios:
  - Intents igual a 129.
  - Presencia: tipo 2 y texto igual a `STATUS`.
  - Un segundo `Ready` no repite el paso 2 (contador de llamadas).
  - Apagado: los seis pasos ocurren en orden (mocks que anotan la secuencia).
  - Apagado con un paso que tarda más de 10 s: sale con 0 y deja un aviso.
  - Interacción sin `guild_id`: no llega al router.
- Paridad con Node: los valores de intents y de presencia se comparan con los de `src/bot.js`.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.
- Manual en el servidor de prueba: arranque, Ctrl+C y corte de red.

## Criterio de aceptación

- `cargo run -p bot` conecta con el token de prueba. El bot aparece en línea con el texto de `STATUS`.
- El log indica un solo shard (0 de 1).
- Ctrl+C sale con código 0 en menos de 10 s. Las sesiones quedan guardadas (S-19).
- Un corte de red de 30 s no tira el proceso. Después, `/help` responde (S-25).
- Un token inválido sale con código 1 y deja el motivo en el log.
- `rust/` no contiene `ShardingManager` ni `broadcastEval`.
- `cargo test` pasa con las pruebas de la lista.
- Ningún archivo pasa de 250 líneas.

## Decisiones

1. Caché: `twilight-cache-inmemory` 0.17 con la feature `permission-calculator`, limitada a guilds, canales, miembros, roles y estado de voz. S-23 saca de ahí el canal de voz y los permisos.
2. Señales de cierre: Ctrl+C, Ctrl+Break y el cierre de la consola. Node solo escucha SIGINT y SIGTERM.
3. Reinicio tras una caída: el proceso sale con código 1. Un `start.bat` lo vuelve a lanzar tras 5 s. Node reinicia el shard con `respawn`. Rust no tiene gestor de shards.
4. Pánicos: un `set_hook` registra el pánico. El supervisor de tareas pide el apagado con código 1.
5. Variables de shards (`SHARDS`, `SHARDING_MANAGER`, `SHARDING_MANAGER_MODE`): se ignoran, con aviso en el log.
6. `pot.js`: Rust no lo arranca hasta que F7 mida si hace falta. Ver la decisión F4 (a) en S-15.
7. Binarios hasta F7: el arranque localiza yt-dlp, ffmpeg y la DLL de Vosk con las variables de S-32 (`YTDLP_PATH`, `FFMPEG_PATH`, `BIN_DIR`). No descarga nada hasta que se implemente el instalador de S-32 en F7.

## Preguntas abiertas

- Ninguna en esta spec. `start.bat` lo especifica S-32.
