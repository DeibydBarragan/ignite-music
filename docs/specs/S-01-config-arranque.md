# Spec S-01: Config y arranque

- Estado: borrador
- Fase: F1
- Módulo: M1
- Origen en Node (solo lectura): `index.js` (arranque), `src/config.js` (todo el archivo), `src/setup/preflight.js` (`run`), `src/setup/preflight-checks.js` (`ensureEnvFile`, `validateProxy`, `checkToken`, `canWrite`), `src/setup/report.js`, `src/setup/setup-error.js`, `.env.example`.

## Objetivo

Leer `.env` con los mismos nombres y valores por defecto que Node. Revisar lo necesario antes de conectar a Discord. Si algo falla, explicarlo y salir con código 1.

No se porta aquí:

- El arranque de Node: versión de Node y `npm install` (`src/setup/bootstrap.js`).
- La comprobación de yt-dlp y ffmpeg (S-33).
- Supabase (S-30), Vosk (S-05) y las librerías de voz de Node.
- Registro de comandos (S-22), túnel (S-29) y shards (S-21). Van después de esta spec.

Hay un solo shard. Las variables de shards se leen, pero solo `TOTAL_SHARDS` tiene una regla.

## Archivos

- Crear: `rust/crates/nucleo/Cargo.toml`
- Crear: `rust/crates/nucleo/src/lib.rs`
- Crear: `rust/crates/nucleo/src/config.rs` (carga de `.env` y struct `Config`)
- Crear: `rust/crates/nucleo/src/env.rs` (parseo de texto, enteros, booleanos, listas y color)
- Crear: `rust/crates/nucleo/src/preflight.rs` (reglas de arranque)
- Crear: `rust/crates/nucleo/src/token.rs` (consulta del token a Discord)
- Crear: `rust/crates/nucleo/src/report.rs` (salida tipo checklist)
- Crear: `rust/crates/nucleo/src/error.rs` (`ArranqueError`)
- Crear: `rust/crates/nucleo/tests/config_paridad.rs`
- Crear: `rust/crates/nucleo/tests/preflight.rs`
- Crear: `rust/crates/nucleo/tests/data/env-casos/` (un `.env` por caso)
- Crear: `scripts/config-paridad-ref.js` (genera la referencia con Node; debe pasar `npm run lint`)
- Modificar: `rust/crates/bot/Cargo.toml` (dependencia `nucleo`)
- Modificar: `rust/crates/bot/src/main.rs` (llama al arranque)
- No tocar: `index.js`, `src/`, `test/`, `.env.example` y el `.env` del usuario.

El crate `nucleo` no estaba en S-31. Ver Preguntas abiertas.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada: el archivo `.env` de la raíz del proyecto y las variables del entorno. Una variable del entorno gana sobre la misma variable de `.env`, como en dotenv.
- Salida: `Config`, de solo lectura, con los campos de `.env.example` y de `src/config.js`. Tipos: texto, entero, booleano, lista y color (entero RGB).
- Errores: `ArranqueError { problemas }`, con el texto `The bot was not started: N problem(s) above must be fixed first.`. El programa sale con código 1. Un error inesperado también sale con código 1.

## Comportamiento

1. Si `<raíz>/.env` no existe y no hay `DISCORD_TOKEN` en el entorno, copia `.env.example` a `.env`. Después falla con `A .env file was created for you, but it still needs your bot token`. Nunca sobrescribe un `.env` existente.
2. Texto: quita los espacios del inicio y del final. Si queda vacío, usa el valor por defecto.
3. Entero: lee los dígitos iniciales, como `parseInt` de JS (`12abc` da 12). Sin dígitos, usa el valor por defecto. Después aplica los límites:

| Variable | Por defecto | Mínimo | Máximo | Nota |
| :--- | :--- | :--- | :--- | :--- |
| `DEFAULT_VOLUME` | 100 | 1 | 100 | |
| `MAX_QUEUE_SIZE` | 500 | 1 | sin límite | |
| `MAX_PLAYLIST_SIZE` | 200 | 1 | sin límite | |
| `LEAVE_ON_EMPTY` | 60 | 0 | sin límite | segundos |
| `LEAVE_ON_FINISH` | 120 | 0 | sin límite | segundos |
| `PANEL_REFRESH` | 20 | 0 | sin límite | segundos |
| `YTDLP_CONCURRENCY` | 4 | 1 | 32 | |
| `YTDLP_TIMEOUT` | 30 | 5 | sin límite | segundos |
| `DNS_LOOKUP_TIMEOUT_MS` | 1500 | 100 | sin límite | ver Preguntas |
| `API_PORT` | 3001 | 1024 | 65535 | |
| `API_RATE_LIMIT_PER_MINUTE` | 120 | 1 | 100000 | |
| `VOICE_SILENCE_DURATION_MS` | 500 | 400 | 3000 | |
| `VOICE_MIN_RMS` | 200 | 0 | 5000 | |
| `VOICE_GROQ_COOLDOWN_MS` | 2000 | 0 | 30000 | |

4. Booleano: `1`, `true`, `yes` y `on` son verdadero, sin distinguir mayúsculas. Cualquier otro valor no vacío es falso. Vacío usa el valor por defecto. Por defecto verdadero: `API_ENABLED`, `VOICE_ENABLED`, `VOICE_OFFLINE_GATING`, `VOICE_TTS_FEEDBACK_ENABLED`. Por defecto falso: `DEBUG`, `AUTO_TUNNEL`, `VOICE_ALLOW_BOT_INPUT`.
5. Lista: separada por comas, sin espacios sobrantes ni vacíos. Vacía usa el valor por defecto. Aplica a `PROXY_URL` y `API_ALLOWED_ORIGINS`.
6. Color: `EMBED_COLOR` debe ser `#RRGGBB`. Si no lo es, usa `#5865F2`. Se guarda como entero.
7. `VOICE_DEFAULT_WAKE_WORD` se guarda en minúsculas.
8. `DEV_USER_IDS`: lista. Si está vacía, usa `NEXT_PUBLIC_DEV_USER_IDS`. Si también está vacía, usa el id fijo de `src/config.js` (ver Preguntas).
9. `TOTAL_SHARDS`: `auto` y `1` son válidos. Cualquier otro valor es un error de arranque. `SHARDS`, `SHARD_MODE`, `SHARD_LIST`, `SHARD_RESPAWN`, `SHARD_SPAWN_DELAY`, `SHARD_SPAWN_TIMEOUT` y `SHARDING_MANAGER` se leen y no tienen efecto.
10. Preflight, en este orden. Cada fallo imprime su mensaje y suma un problema. Si se crea el `.env` (regla 1), se para ahí.
    a. `DISCORD_TOKEN` vacío: `DISCORD_TOKEN is empty`, con los pasos de `TOKEN_STEPS`.
    b. Formato del token: `^[\w-]{20,}\.[\w-]{4,}\.[\w-]{20,}$`. Si no cumple: `DISCORD_TOKEN does not look like a bot token`.
    c. Token en Discord: `GET https://discord.com/api/v10/applications/@me` con `Authorization: Bot <token>` y 10 s de límite.
       - 401: `Discord rejected DISCORD_TOKEN (wrong or reset token)`.
       - Otro código o sin red: si `CLIENT_ID` está vacío, falla con `Could not reach Discord to check the token`. Si tiene valor, avisa con `Could not reach Discord to check the token, trying anyway`.
       - Éxito: imprime `Discord token` con el nombre del bot y el id de la aplicación. Guarda el id en `Config`. Node lo escribe en `process.env.CLIENT_ID`. Rust no escribe en el entorno.
    d. `CLIENT_ID` distinto del id de la aplicación: aviso.
    e. `GUILD_ID` con valor que no cumple `^\d{17,20}$`: falla.
    f. Cada `PROXY_URL` debe leerse como URL, con esquema `http`, `https`, `socks4`, `socks5` o `socks5h`, y con host. Si no: `PROXY_URL "<valor>" is not a valid proxy address`.
    g. `COOKIES_FILE`: la ruta se resuelve desde la raíz. Si no existe, falla. Si existe, guarda la ruta absoluta. Si no hay `COOKIES_FILE` pero sí `COOKIES_FROM_BROWSER`, el nombre antes de `:` debe ser brave, chrome, chromium, edge, firefox, opera, safari, vivaldi o whale.
    h. `DATA_DIR` debe ser escribible. Se crea la carpeta, se escribe un archivo de prueba y se borra.
    i. Con `API_ENABLED`: `API_SECRET` debe tener 32 caracteres o más. Si no, falla. Si `API_ALLOWED_ORIGINS` está vacío, solo informa.
    j. Sin claves de Spotify: solo informa.
    k. `YTDLP_JS_RUNTIME`: si está vacía, avisa con `[WARN]` y yt-dlp corre sin `--js-runtimes`. Si tiene valor, debe tener la forma `runtime:ruta`, por ejemplo `node:<ruta>`, y la ruta debe existir. Si no, falla.
11. Salida en formato de lista: `[ OK ]`, `[INFO]`, `[WARN]` y `[FAIL]`. Los textos son los de Node, en inglés. Hay color solo si la salida es una terminal y `NO_COLOR` no está definido. `OK` e `INFO` van a la salida estándar. `WARN` y `FAIL` van a la salida de error.
12. Con problemas, imprime `The bot was not started: N problem(s) above must be fixed first.` y `Fix them and start the bot again. Every step is also explained in README.md.` en la salida de error. Después sale con código 1.
13. Sin problemas, sigue el arranque: registro de comandos (S-22), túnel (S-29) y shards (S-21).

## Tests

- Unitarios: parseo de cada tipo (límites, `12abc`, booleanos raros, listas con espacios, color inválido, wake word en minúsculas, `DEV_USER_IDS` en cadena). Validación de token, proxy, `GUILD_ID`, cookies y `DATA_DIR`. `API_SECRET` de 31 caracteres falla y de 32 pasa.
- Token: pruebas contra un servidor HTTP local, no contra Discord. Casos: 200, 401, 500 y sin conexión. Son los de `checkToken` en `test/setup.test.js`.
- `.env`: se crea desde el ejemplo si falta, y nunca se sobrescribe. Es el caso de `test/setup.test.js`.
- Paridad con Node: `scripts/config-paridad-ref.js` lee cada archivo de `tests/data/env-casos/` en un proceso propio, carga `src/config.js` y guarda los valores en JSON. El test de Rust hace lo mismo con `Config` y compara campo por campo. Los casos incluyen vacíos, enteros fuera de rango, `12abc`, booleanos raros, listas con espacios y color inválido.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Con el mismo `.env`, `Config` de Rust y `src/config.js` dan los mismos valores en todos los campos de los casos de paridad.
- Un `.env` vacío se crea desde el ejemplo. El programa sale con código 1 y pide el token.
- Un token rechazado sale con código 1 y el mensaje de Node.
- Sin red y con `CLIENT_ID` puesto, avisa y sigue.
- Cada regla del preflight tiene una prueba, y sus mensajes son los de Node.
- `cargo test` pasa sin token ni red.

## Preguntas abiertas

- Raíz del proyecto. Node usa la carpeta padre de `src/`. Propuesta: la carpeta desde la que corre el binario. `start.bat` hace `cd` (S-33).
- `.env` malformado. El dotenv de Node ignora las líneas malas. Propuesta: lo mismo, con aviso en el log.
- Valores por defecto distintos. `.env.example` pone `VOICE_ENABLED=false` y `VOICE_TTS_FEEDBACK_ENABLED=false`, pero el código usa `true` si la variable falta. Propuesta: el ejemplo no cambia y el código manda solo cuando la variable falta.
- `DEV_USER_IDS` trae un id de desarrollador fijo en el código. ¿Se mantiene en Rust?
- `DNS_SERVERS` y `DNS_LOOKUP_TIMEOUT_MS` sirven al parche de DNS de Node (`src/core/dns-fix.js`). Propuesta: no portarlos, salvo que haga falta.
- Título de arranque: Node imprime `Beatra startup check`. El resto del bot dice Ignite. ¿Se cambia?
- Crate `nucleo`: no estaba en S-31. Propuesta: crearlo para M1.
- `TOTAL_SHARDS` distinto de `auto` o `1`: propuesta, error de arranque. Confirmar.
