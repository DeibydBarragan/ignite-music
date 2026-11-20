# Spec S-12: Registro de eventos de voz y CSV

- Estado: implementado
- Fase: F3
- Módulo: M2
- Origen en Node (solo lectura): `src/voice/event-log.js` (`VoiceEventLogStore`: `record`, `getLogs`, `clear`), `src/voice/csv-exporter.js` (`VoiceCsvExporter`: `add`, `flush`, `flushAll`; `buildCsv`, `toCsvCell`, `CSV_COLUMNS`), `src/bot.js` (`flushAll` al apagar), `test/voice-csv.test.js`.

## Objetivo

Guardar en memoria los últimos 100 eventos de voz de cada servidor, para la consola en vivo. Publicar cada evento por SSE. Escribir CSV en lotes de 50 y al apagar el bot, solo en los servidores que activan el CSV. Borrar los CSV con más de 30 días.

No incluye quién genera los eventos (S-08 y S-10) ni la ruta SSE (S-28).

## Archivos

- Crear: `rust/crates/voice/src/registro_voz.rs` (anillo de 100 eventos por servidor y publicación)
- Crear: `rust/crates/voice/src/csv_voz.rs` (CSV por lotes y retención)
- Crear: `rust/crates/voice/tests/registro_voz.rs`
- Crear: `rust/crates/voice/tests/csv_voz.rs`
- Crear: `rust/crates/voice/tests/data/csv-casos.json` (generado con Node)
- Crear: `scripts/csv-paridad-ref.js` (debe pasar `npm run lint`)
- No tocar: `src/voice/event-log.js`, `src/voice/csv-exporter.js`, `test/voice-csv.test.js`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada de `registrar(entrada)`:
  - `servidor` (texto), `usuario` (opcional), `bytes_audio` (número).
  - `vosk_oyo` (texto), `palabra_vista` (booleano), `palabra` (opcional), `motivo_puerta` (opcional).
  - `groq_usado` (booleano), `texto_groq` (opcional), `ms_groq` (opcional).
  - `comando` (opcional), `consulta` (opcional), `mensaje_tts` (opcional).
  - `estado`, por defecto `filtered_out`.
- Salida de `registrar`: la entrada normalizada, con `id` (`vlog_<milisegundos>_<5 caracteres>`) y `timestamp` (ISO 8601).
- Salida de `obtener(servidor, limite = 50)`: hasta `limite` eventos, el más reciente primero.
- Entrada de `fijar_csv(servidor, activo)`: la llama S-30 al cargar o cambiar el ajuste del servidor. Por defecto, el CSV está desactivado en todos los servidores.
- El procesador de voz lee el opt-in persistido en `guild_voice_settings.tts_templates._csv` y lo aplica antes de registrar fragmentos. Ese valor es metadata interna del servidor y las ediciones de plantillas lo conservan. El audio PCM no se escribe en el CSV.
- Columnas del CSV, en este orden: `id`, `timestamp`, `guildId`, `userId`, `audioBytes`, `voskHeard`, `wakeWordSpotted`, `matchedWakeWord`, `gateReason`, `groqEngaged`, `groqText`, `groqElapsedMs`, `commandId`, `query`, `ttsMessage`, `status`. Son los nombres de Node, no los de Rust.
- Errores: un fallo al escribir el CSV se registra como aviso. No detiene el registro en memoria.

## Comportamiento

1. Anillo por servidor de 100 eventos. Al llegar el 101.º, se borra el más antiguo.
2. `obtener` devuelve los más recientes primero. Si `limite` es mayor que los guardados, devuelve todos.
3. `limpiar(servidor)` vacía el anillo de ese servidor.
4. Cada evento registrado se publica por SSE en el servidor, con el nombre `voice.log` (S-28). Si el hub SSE no existe, se ignora.
5. Si el servidor tiene el CSV activo, cada evento se pasa al exportador. Un fallo del exportador se ignora.
6. Normalización, igual que Node en `event-log.js`: `id` es `vlog_<milisegundos>_<5 caracteres>` y `timestamp` es ISO 8601. `servidor` se guarda como texto. `userId` sin dato es nulo. `bytes_audio` que no sea número es 0. `vosk_oyo` sin dato es cadena vacía. Los booleanos que falten son falso. `palabra`, `motivo_puerta`, `texto_groq`, `ms_groq`, `comando`, `consulta` y `mensaje_tts` sin dato son nulos (`ms_groq` en 0 también es nulo). `estado` que falte es `filtered_out`.
7. Lote CSV: cuando un servidor con el CSV activo acumula 50 eventos nuevos, se escribe un archivo con esos 50 y su lote queda vacío.
8. Nombre del archivo: `<servidor>-<fecha ISO con : y . cambiados por ->.csv`, dentro de `DATA_DIR/voice-logs/`. La carpeta se crea si falta.
9. Contenido: cabecera con las 16 columnas, una fila por evento en orden de llegada, y salto de línea al final.
10. Celdas:
    a. Nulo o indefinido es vacío.
    b. Si el texto tiene comas, comillas o saltos de línea, va entre comillas dobles, y las comillas dentro se duplican.
    c. Las celdas de texto libre (`voskHeard`, `groqText`, `query`, `ttsMessage` y `matchedWakeWord`) que empiezan por `=`, `+`, `-`, `@`, tabulador o retorno de carro llevan un apóstrofe delante. Las demás columnas no cambian.
11. Apagado: `flush_all()` escribe todos los lotes parciales de los servidores con CSV activo, sin importar su tamaño, y los vacía. Node lo llama al apagar (`src/bot.js`).
12. Un lote parcial no se escribe antes del apagado, ni antes de llegar a 50.
13. Retención: al arrancar, y después cada 24 horas, se borran los archivos de `DATA_DIR/voice-logs` cuya fecha de modificación tiene más de 30 días. Node no borra nunca.

## Tests

- Unitarios de `test/voice-csv.test.js`:
  - `toCsvCell` escapa comas, comillas y saltos de línea.
  - `buildCsv` pone la cabecera y una fila por entrada.
  - El exportador escribe un archivo cada 50 registros.
  - El registro dispara el volcado por el exportador inyectado.
  - `flushAll` vuelca los lotes parciales de todos los servidores.
- Propios de Rust:
  - El anillo descarta el 101.º evento.
  - `obtener` devuelve el orden correcto.
  - `limpiar` vacía el anillo.
  - La publicación SSE usa el nombre `voice.log`.
  - Un directorio de solo lectura no tumba el registro.
  - Un servidor sin CSV activo no genera archivos.
  - Una celda de texto que empieza por `=` lleva apóstrofe, y las columnas de números no cambian.
  - La retención borra los archivos con más de 30 días y deja los demás.
- Paridad con Node: `scripts/csv-paridad-ref.js` genera con Node el CSV de una lista fija de entradas, en `tests/data/csv-casos.json`. La lista incluye comas, comillas, saltos de línea, nulos y textos largos. El test de Rust compara byte a byte, salvo las celdas con apóstrofe de la regla 10, que se prueban aparte.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- El CSV de Rust es igual, byte a byte, al de Node para `csv-casos.json`, salvo las celdas con apóstrofe de la regla 10.
- Cada 50 eventos de un servidor con CSV activo aparece exactamente un archivo nuevo. Un servidor sin CSV activo no genera ninguno.
- `flush_all` escribe los lotes parciales antes de cerrar el proceso.
- La retención borra los archivos con más de 30 días.
- `cargo test` pasa usando una carpeta temporal, no la carpeta de datos.
- Ningún archivo pasa de 250 líneas.

## Decisiones

- Inyección de fórmulas: se aplica el apóstrofe de la regla 10. Node no lo hace, así que el CSV de Rust se aparta de Node en esas celdas.
- Activación y retención: el CSV se escribe solo si el servidor lo activa (S-30), y no está activo por defecto. Los archivos con más de 30 días se borran (regla 13). Node escribe siempre y no borra nunca.
- Identificador: se mantiene el formato de Node, `vlog_<milisegundos>_<5 caracteres>` con `Date.now()` y un número aleatorio en base 36 (decisión del plan, 9.7). El reloj de Rust da los milisegundos igual que Node; los 5 caracteres aleatorios son alfanuméricos en minúsculas.

## Preguntas resueltas

- Identificador: ver Decisiones. La llamada de `fijar_csv` al cargar o cambiar el ajuste es de S-30; esta spec solo expone la función.
