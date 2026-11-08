# Spec S-15: Qué hacer con innertube, relay y pot.js

- Estado: borrador
- Fase: F4
- Módulo: M3
- Origen en Node (solo lectura): `src/music/innertube.js` (`searchVideosDirect`, `searchPlaylistsDirect`, `getRelatedMixDirect`, `getPlaylistVideosDirect`), `src/music/innertube-util.js`, `src/music/relay.js` (`StreamRelay`, `trimInfo`, `parseRange`, `getRelay`), `src/music/pot.js` (`currentPoToken`, `currentPoTokenAsync`, `startPoTokenRenewal`), `src/setup/launcher.js` (`StreamRelay.cleanAll`), `src/setup/preflight.js` (arranque de pot.js).
- Consumidores: `src/music/plugins.js`, `src/music/suggest.js`, `src/music/playlists.js`, `src/music/ytdlp.js`, `src/music/events.js` (`manager.relay.errorFor`).

## Objetivo

Decidir qué se porta de innertube, del relay y de pot.js. El plan dice que solo se usa yt-dlp, sin los tres. Esta spec confirma esa decisión y marca lo que cambia con ella.

No escribe código. Define qué se mide en F7 antes de decidir si hace falta algo más.

## Archivos

- Ninguno de código en esta spec.
- Las mediciones de F7 se anotan en el plan de paridad de S-33.

## Contrato

- Entrada: las decisiones de este documento.
- Salida: lista de funciones retiradas y lo que las sustituye en Rust.

## Comportamiento

1. innertube.js (321 líneas) es un cliente HTTP directo de la API de YouTube. Lo usan:
   - La búsqueda de videos (`plugins.js`).
   - La búsqueda de listas (`plugins.js`).
   - La mezcla de relacionados (`plugins.js`).
   - Los videos de una lista (`playlists.js`).
   - Las sugerencias (`suggest.js`).
   Decisión: se retira, como dice el plan. Sustitutos: `ytsearch` y la lista plana con yt-dlp, y `watch?v=<id>&list=RD<id>`, que ya están en S-13 y S-14. Esto cambia el tiempo de respuesta de las búsquedas. Según el comentario del propio archivo, innertube responde en pocos cientos de milisegundos, y yt-dlp tarda más por el arranque del proceso.
2. Cabeceras fijas de innertube: `CLIENT_VERSION = 2.20240101.00.00` y un User-Agent de Chrome 122. Si YouTube deja de aceptarlas, innertube falla y el bot pasa a yt-dlp (`directEntries`). Riesgo bajo mientras exista ese respaldo.
3. relay.js (413 líneas) es un servidor HTTP local en 127.0.0.1. yt-dlp descarga el formato elegido y ffmpeg lee el audio desde el relay. Incluye:
   - Peticiones de rango (206) y reconexión sin volver a extraer.
   - `warm()`, que empieza la descarga antes de reproducir.
   - `errorFor(url)`, que guarda la última salida de error de yt-dlp, para el mensaje al usuario.
   - Límite de 300 entradas y 6 horas de vida.
   - Una carpeta por proceso en `DATA_DIR/relay/<pid>`, que se limpia al arrancar (`StreamRelay.cleanAll`).
   Decisión: se retira, como dice el plan. Sustituto: songbird lee la entrada directamente. Hay que confirmar que las cabeceras, las cookies y el proxy llegan a la entrada de songbird (S-16). Plan B: portar el relay solo si F7 muestra errores 403 de YouTube.
4. Pérdida que hay que aceptar sin relay: el texto de error de yt-dlp (`errorFor`) deja de llegar a ffmpeg. Sustituto: el runner de S-13 ya captura el stderr de yt-dlp. Si la entrada pasa por ese runner, el error vuelve a estar disponible.
5. pot.js (157 líneas) renueva cada 30 minutos un token PO (Proof of Origin) y lo pasa a yt-dlp. El generador es un script de Node (`bgutil-ytdlp-pot-provider`, `generate_once.js`), que se ejecuta con `process.execPath`. La renovación tiene un margen de 1 hora antes de que el token caduque. Si falla, yt-dlp sigue sin token.
6. Conflicto con el plan: el plan retira pot.js. El código lo usa para que YouTube no pida verificación desde IPs de centros de datos. Sin pot.js, el token queda en `YOUTUBE_PO_TOKEN` (fijo) o en las cookies, y los dos caducan.
7. Decisión para pot.js: no se retira en F4. Se mide en F7 si YouTube pide verificación en el servidor de prueba, con y sin token renovado. Si la pide, decidir entre cookies, token fijo u otro servicio de token. Si el generador sigue siendo de Node, la máquina necesita Node después de F8.

## Tests

- Mediciones de F7, en el plan de paridad de S-33:
  - Búsquedas con yt-dlp: tiempo mediano y percentil 95, en el mismo servidor de prueba que Node.
  - Errores 403 y de verificación de bot durante 24 horas de uso de prueba, con y sin token renovado.
  - Número de procesos de yt-dlp por canción.
- Pruebas de regresión: las de S-13 y S-14 cubren los sustitutos.
- Gates: `cargo test` y `npm test` sin cambios.

## Criterio de aceptación

- Cada función retirada tiene un sustituto escrito en esta spec o en S-13 y S-14.
- La decisión sobre pot.js se toma con los datos de F7, no antes.
- La lista de retiradas coincide con la del plan (innertube, relay y pot.js), salvo pot.js, que queda abierto.

## Decisiones

- pot.js no se retira en F4. Se mide en F7 (ver Comportamiento 7).
- innertube se retira. Su cabecera fija de cliente deja de importar.
- Relay: se confirma su retirada si F7 no muestra errores 403 de YouTube. Si los muestra, se porta (Plan B).

## Preguntas abiertas

- Generador de tokens: si pot.js se mantiene, ¿se sigue usando el script de Node o se busca otra implementación? Un script de Node obliga a tener Node después de F8.
