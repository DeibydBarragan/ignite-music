# Spec S-32: Binarios y arranque en Windows

- Estado: borrador
- Fase: F7
- Módulo: M8
- Origen en Node (solo lectura): `src/core/binaries/index.js`, `src/core/binaries/installers/ytdlp.js` (`ensureYtDlp`, `updateYtDlp`, `installYtDlp`, `assetChecksum`), `src/core/binaries/installers/ffmpeg.js` (`ensureFfmpeg`), `src/core/binaries/installers/cloudflared.js` (`ensureCloudflared`), `src/core/binaries/downloader.js` (`downloadToFile`), `src/core/binaries/probe.js` (`probe`), `src/voice/model-downloader.js`, `src/setup/preflight.js` (bloque de herramientas de audio), `scripts/install-binaries.js` (postinstall de npm) y `start.bat`.

## Objetivo

Dejar listos para Rust los binarios y modelos que usa el bot: yt-dlp, ffmpeg, cloudflared, la DLL de Vosk y sus modelos. Lanzar el bot desde `start-rust.bat`, que lo vuelve a levantar tras una caída.

Solo Windows (decisión 7 del plan). No incluye la carga de Vosk (S-05), la lógica de yt-dlp (S-13) ni el túnel (S-29).

## Decisiones

- Los binarios van en `BIN_DIR` (por defecto `bin/`), en las mismas rutas que Node.
- El código va en el crate `bot` (`rust/crates/bot/src/binarios/`). No se crea un crate nuevo. Si se aprueba `nucleo` (S-01), este código se mueve allí.
- No hay paso de instalación aparte. El bot asegura los binarios al arrancar, como hace `preflight.js`. El postinstall de npm no tiene equivalente.
- Sin yt-dlp o sin ffmpeg, el arranque falla con código 1, como en Node.
- `start.bat` de Node no cambia hasta F8. Rust se lanza con `start-rust.bat`.
- ffmpeg: el zip `6.1.1-essentials` de gyan.dev, con el SHA-256 que publica gyan.dev. Si esa versión ya no se publica, se usa el essentials actual y se repiten las pruebas de filtros de S-18.
- cloudflared, la DLL de Vosk y sus modelos: el SHA-256 se fija tras revisar a mano el archivo oficial. Cada descarga se verifica contra ese hash.
- yt-dlp con `YTDLP_DOWNLOAD_BASE`: se verifica contra `SHA2-256SUMS` de esa base si existe. Si no existe, avisa y no verifica.
- cloudflared: Rust borra solo la copia de `BIN_DIR`. Una copia fuera de `BIN_DIR` que no responde se ignora.
- Carpeta de binarios en la prueba paralela: el bot de Rust de prueba usa un `BIN_DIR` propio, fuera de `bin/` (S-33). Así dos procesos no escriben el mismo `yt-dlp.exe`.

## Archivos

- Crear: `rust/crates/bot/src/binarios/mod.rs`: orden de arranque y rutas resultantes.
- Crear: `rust/crates/bot/src/binarios/descarga.rs`: descarga a fichero con tamaño mínimo y máximo, SHA-256 opcional y tiempo límite.
- Crear: `rust/crates/bot/src/binarios/ytdlp.rs`: instalación, verificación, extracción y actualización de cada 24 horas.
- Crear: `rust/crates/bot/src/binarios/ffmpeg.rs`: búsqueda y descarga.
- Crear: `rust/crates/bot/src/binarios/cloudflared.rs`: búsqueda y descarga.
- Crear: `rust/crates/bot/src/binarios/vosk.rs`: DLL y modelos.
- Crear: `rust/crates/bot/src/binarios/sonda.rs`: ejecuta `--version` o `-version` con tiempo límite.
- Crear: `rust/crates/bot/tests/binarios.rs`: pruebas con un servidor HTTP local.
- Crear: `start-rust.bat` en la raíz de `ignite-music`.
- Modificar: `rust/crates/bot/Cargo.toml`: cliente HTTP con TLS (crate a elegir al añadirlo), `zip` y `sha2`, con versiones fijadas al añadirlas.
- No tocar: `src/core/binaries/`, `src/voice/model-downloader.js`, `scripts/install-binaries.js` y `start.bat`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

Entradas (S-01): `BIN_DIR` (por defecto `bin`), `DATA_DIR`, `YTDLP_PATH`, `YTDLP_DOWNLOAD_BASE`, `FFMPEG_PATH` y `AUTO_TUNNEL`.

Rutas resultantes:

- yt-dlp: `YTDLP_PATH`, o `BIN_DIR/yt-dlp-dist/yt-dlp.exe`.
- ffmpeg: `FFMPEG_PATH`, o `BIN_DIR/ffmpeg.exe`.
- cloudflared: `BIN_DIR/cloudflared.exe`, con las otras rutas de la regla 7.
- Vosk: `BIN_DIR/vosk/libvosk.dll`.
- Modelos: `DATA_DIR/models/vosk-model-small-en-us-0.15/` (listo con `.ready` y `am/final.mdl`) y `DATA_DIR/models/vosk-model-small-es/` (listo con `am/final.mdl`).
- Marca de actualización de yt-dlp: `DATA_DIR/yt-dlp.checked`.

Salida: una ruta por binario, y una línea de resumen por binario en el log, como `[ OK ] yt-dlp <versión>`.

Errores de arranque:

- Sin yt-dlp: `yt-dlp is missing (needed for YouTube)`, con el paso de instalación manual, como `preflight.js`.
- Sin ffmpeg: `ffmpeg is missing (needed to play any audio)`, con el paso de instalación manual.
- Los motivos van al log con los textos de Node: `No working yt-dlp found. YouTube will not play. Download it from https://github.com/yt-dlp/yt-dlp/releases and set YTDLP_PATH.` y `No working ffmpeg found. Install ffmpeg or set FFMPEG_PATH.`

## Comportamiento

1. Orden de arranque, después de la configuración (S-01) y antes de conectar a Discord:
   1. yt-dlp. Si `YTDLP_PATH` está puesto y no responde, falla. No busca otra copia. Si no está puesto, prueba `BIN_DIR/yt-dlp-dist/yt-dlp.exe`. Si ese archivo no responde, lo instala de nuevo. Si no existe o la instalación falla, prueba `yt-dlp` del PATH. Si nada responde, falla con código 1.
   2. Con yt-dlp encontrado, comprueba la actualización (regla 4).
   3. ffmpeg. Si `FFMPEG_PATH` está puesto y no responde, falla. Si no, prueba `BIN_DIR/ffmpeg.exe`. Si no existe, lo descarga (regla 5). Si nada responde, prueba `ffmpeg` del PATH. Si nada responde, falla con código 1.
   4. Vosk: DLL y modelos (regla 6). Si falla la descarga, la puerta de palabra queda cerrada (S-06) y el bot arranca.
   5. cloudflared: solo si `AUTO_TUNNEL` está activo (regla 7).
2. Sonda: ejecuta el binario con `--version` (yt-dlp y cloudflared) o `-version` (ffmpeg), con 60 segundos de tiempo límite. Responde ok si termina bien dentro del tiempo límite y da salida.
3. yt-dlp, instalación: descarga `yt-dlp_win.zip` desde `https://github.com/yt-dlp/yt-dlp/releases/latest/download/` o desde `YTDLP_DOWNLOAD_BASE`. Con la fuente oficial, verifica el SHA-256 contra `SHA2-256SUMS` de la misma carpeta. Con `YTDLP_DOWNLOAD_BASE`, verifica contra `SHA2-256SUMS` de esa base si existe. Si no existe, avisa y no verifica (ver Decisiones). Límites del zip: 256 MiB de archivo, 1 GiB extraído y 5000 entradas. Instala en una carpeta temporal y la pone en `BIN_DIR/yt-dlp-dist/` al final. Borra los binarios sueltos de versiones antiguas (`yt-dlp` y `yt-dlp.exe` en `BIN_DIR`). Al terminar escribe la fecha en `DATA_DIR/yt-dlp.checked`.
4. yt-dlp, actualización: si la marca tiene menos de 24 horas, no consulta. Si no, lee la última versión de la redirección de `releases/latest`, sin API de GitHub. Si es distinta de la instalada, instala de nuevo (regla 3). Si la consulta funciona, guarda la fecha en la marca. Si la consulta falla, avisa y sigue con la copia actual.
5. ffmpeg, descarga: el build es el mismo que usa Node, `6.1.1-essentials` de gyan.dev, empaquetado por `ffmpeg-static` 5.3.0 (release `b6.1.1`). Descarga el zip a un temporal, verifica su SHA-256 publicado (ver Decisiones), extrae `ffmpeg.exe` y lo deja en `BIN_DIR/ffmpeg.exe`. Si falla, avisa y prueba el PATH.
6. Vosk:
   - DLL: `vosk-win64-0.3.45.zip` desde `https://github.com/alphacep/vosk-api/releases/download/v0.3.45/`, con mínimo de 5 MiB. Extrae `libvosk.dll` a `BIN_DIR/vosk/`.
   - Modelos: si falta `am/final.mdl`, baja `vosk-model-small-en-us-0.15.zip` (unos 39 MB) y `vosk-model-small-es-0.42.zip` (unos 45 MB) desde `https://alphacephei.com/vosk/models/` a `DATA_DIR/models/`. Cada modelo queda en su carpeta, `vosk-model-small-en-us-0.15/` y `vosk-model-small-es/`, con los nombres de Node.
   - DLL y modelos usan la regla 8 y verifican el SHA-256 fijado (ver Decisiones).
7. cloudflared, solo con `AUTO_TUNNEL`:
   - Busca, en este orden: `BIN_DIR/cloudflared.exe`, `cloudflared.exe` en la raíz del proyecto, `C:\Program Files (x86)\cloudflared\cloudflared.exe` y `C:\Program Files\cloudflared\cloudflared.exe`. Usa el primero que responde a `--version`.
   - Si no hay ninguno, prueba `cloudflared` del PATH.
   - Si nada responde, descarga `cloudflared-windows-amd64.exe` desde `https://github.com/cloudflare/cloudflared/releases/latest/download/` a `BIN_DIR/cloudflared.exe`, con mínimo de 10 MiB y máximo de 128 MiB. Verifica contra el SHA-256 fijado (ver Decisiones).
   - Si nada funciona, avisa. El túnel no arranca (S-29).
   - Un candidato fuera de `BIN_DIR` que no responde se ignora y no se borra (ver Decisiones).
8. Descarga común (`descarga.rs`): escribe a un fichero temporal de la misma carpeta y lo renombra al terminar. Mínimo por defecto de 1 MiB, máximo de 512 MiB y tiempo límite de 10 minutos, como en Node. Calcula el SHA-256 mientras escribe. Si hay error de HTTP, de tamaño, de hash o de tiempo, borra el temporal y falla.
9. `start-rust.bat`, en la raíz:
   - Usa `title Beatra` y `cd /d "%~dp0"`, como `start.bat`.
   - Si falta `rust\target\release\bot.exe`, avisa con el comando de compilación (`cargo build --release -p bot`, dentro de `rust\`), espera una tecla y sale con código 1.
   - Lanza `rust\target\release\bot.exe`. Si sale con código 1 (caída), espera 5 segundos y lo vuelve a lanzar.
   - Si sale con código 0 (apagado normal), muestra `Bot stopped.` y espera una tecla, como `start.bat`.
10. `start.bat` de Node no cambia. Sigue lanzando `node index.js` hasta F8.

## Tests

- Orden de búsqueda: `YTDLP_PATH` roto falla sin buscar más; un `yt-dlp-dist` roto se instala de nuevo; un `yt-dlp` del PATH se usa cuando no hay copia local. Mismo orden para ffmpeg.
- Descarga, con un servidor HTTP local: 404, respuesta de 100 bytes (menos del mínimo), respuesta de más del máximo, SHA-256 incorrecto y tiempo límite. En todos los casos no queda temporal.
- yt-dlp: un zip con más entradas de las permitidas falla. Un zip válido se instala y escribe la marca. Con `YTDLP_DOWNLOAD_BASE`, pide `SHA2-256SUMS` de esa base. Si no existe, instala y avisa.
- Actualización: una marca de menos de 24 horas no consulta (reloj simulado). Una marca vencida con versión nueva reinstala.
- Vosk: un modelo con `am/final.mdl` no se descarga. El inglés solo se da por listo con `.ready` y `am/final.mdl`.
- cloudflared: un candidato fuera de `BIN_DIR` que no responde no se borra.
- `start-rust.bat`: sin `bot.exe`, avisa y sale con código 1. Con un `bot.exe` que sale con código 1, lo relanza tras 5 segundos. Con código 0, no lo relanza.
- Paridad: los textos de error y el resumen de arranque son los de `preflight.js`.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- En una carpeta `bin/` vacía, `start-rust.bat` deja yt-dlp, ffmpeg y la DLL de Vosk en `BIN_DIR`, y los modelos en `DATA_DIR/models/`, sin pasos manuales.
- Con yt-dlp o ffmpeg quitados y sin copia en el PATH, el arranque dice cuál falta y sale con código 1.
- El bot reproduce con el ffmpeg descargado, sin ffmpeg en el PATH.
- Tras una caída (código 1), `start-rust.bat` vuelve a lanzar el bot en 5 segundos.
- `cargo test` pasa sin red.

## Preguntas abiertas

1. SHA-256 del zip 6.1.1 de gyan.dev. El hash que se tiene (`04E1307997530F9CF2FE35CBA2CA7E8875CA91DA02F89D6C7243DF819C94AD00`) es del `.exe` del paquete de Node, no del zip. No está comprobado en la página de gyan.dev. Se resuelve antes de F7: comprobar que el zip sigue publicado, descargarlo y fijar su hash. Si no está publicado, aplica la decisión de ffmpeg.
2. SHA-256 de cloudflared, de la DLL de Vosk y de sus modelos. Node no verifica estos archivos. Se resuelve antes de F7: descargar cada archivo oficial, revisarlo a mano y fijar su hash.
