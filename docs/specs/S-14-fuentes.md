# Spec S-14: Fuentes (YouTube, Spotify, enlaces directos y otros sitios)

- Estado: borrador
- Fase: F4
- Módulo: M3
- Origen en Node (solo lectura): `src/music/plugins.js` (`YouTubePlugin`), `src/music/plugins-base.js` (`YtDlpBase`, `parseYouTubeUrl`, `youtubeInfo`, `splitYouTubeTitle`, `streamTtl`, `isLyricTitle`, constantes), `src/music/plugins-generic.js` (`GenericPlugin`, `RelayedDirectLinkPlugin`), `src/music/manager.js` (plugins registrados y `SpotifyPlugin`), `test/plugins.test.js`.

## Objetivo

Convertir una URL o un texto en una canción o en una lista, con sus metadatos y su enlace de audio. Cubre YouTube (videos, listas, búsqueda, relacionados y letras), Spotify (metadatos, y audio desde YouTube), enlaces directos y otros sitios que soporta yt-dlp.

No incluye la cola (S-17), los filtros (S-18), ni innertube, que se retira (S-15).

## Archivos

- Crear: `rust/crates/medios/src/youtube.rs` (URL, información, búsqueda y listas)
- Crear: `rust/crates/medios/src/youtube_titulo.rs` (separar artista y título, limpiar sufijos)
- Crear: `rust/crates/medios/src/genericas.rs` (sitios de yt-dlp y enlaces directos)
- Crear: `rust/crates/medios/src/spotify.rs` (metadatos de Spotify)
- Crear: `rust/crates/medios/tests/youtube.rs`, `rust/crates/medios/tests/titulo.rs`, `rust/crates/medios/tests/genericas.rs`, `rust/crates/medios/tests/spotify.rs`
- Crear: `rust/crates/medios/tests/data/titulos-casos.json` (generado con Node)
- Crear: `scripts/titulos-paridad-ref.js` (debe pasar `npm run lint`)
- No tocar: `src/music/plugins*.js`, `test/plugins.test.js`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada de `resolver(url, opciones)`: una URL o un texto de búsqueda, el servidor (para elegir proxy) y el miembro que pide.
- Salida:
  - Canción: `{ fuente, id, nombre, url, es_directo, duracion, miniatura, autor, edad_restringida }`.
  - Lista: `{ fuente, id, nombre, url, canciones }`.
- Errores: `UrlNoSoportada`, `ListaVacia`, `ListaFiltradaVacia` (todas las canciones tienen edad restringida y el canal no es NSFW), `NsfwNoPermitido` y `ResultadoVacio`. Los errores de yt-dlp llegan desde S-13.

## Comportamiento

1. URL de YouTube (`parseYouTubeUrl`):
   a. Hosts aceptados: `youtube.com`, `youtu.be`, `music.youtube.com` y `youtube-nocookie.com`, con y sin `www.`, y también `m.youtube.com`.
   b. En `youtu.be/<id>`, el id es el primer segmento de la ruta. En `/shorts/`, `/live/`, `/embed/` o `/v/` seguidos de un id de 11 caracteres, ese es el id.
   c. Si hay `list=`, es una lista. La excepción es una mezcla (`RD`) con un video a la vez: se trata como video.
   d. Si hay un id de video válido, es un video. Si no, la URL no está soportada.
2. Video: una sola llamada a yt-dlp con `--no-playlist -f bestaudio[acodec=opus]/bestaudio/best`, que da metadatos y flujo a la vez. El resultado se guarda en caché con el TTL de la URL de audio (punto 15).
3. Lista: `--flat-playlist --playlist-end <MAX_PLAYLIST_SIZE>`. Se descartan las entradas sin id válido y las marcadas como privadas o eliminadas. Si no queda ninguna: `EMPTY_PLAYLIST`.
4. Información de una canción (`youtubeInfo`): la URL es `https://www.youtube.com/watch?v=<id>` y la miniatura es `https://i.ytimg.com/vi/<id>/hqdefault.jpg`. Es directo si `is_live` o `live_status` vale `is_live` (duración 0). Edad restringida si `age_limit` es 18 o más.
5. Separar título y artista (`splitYouTubeTitle`):
   a. Quita sufijos entre paréntesis o corchetes: "Official Video", "Official Audio", "Video oficial", "Audio oficial", "Official Lyric Video", "Lyric Video", "Lyrics" y "Visualizer".
   b. Si el canal acaba en " - Topic", el artista es el nombre sin " - Topic".
   c. Si el título tiene " - " y el lado izquierdo coincide con el canal (uno contiene al otro, sin acentos y sin "VEVO" al final), el izquierdo es el artista y el derecho el título.
   d. En otro caso: título limpio y canal como artista.
6. Letras: un título que contenga "lyric" o "lyrics" marca la URL como letra durante 24 horas. Caché de hasta 5000 URL.
7. Búsqueda por texto (`buscar_cancion`): `ytsearch1:<texto>` con `-f bestaudio[acodec=opus]/bestaudio/best`. Caché de 30 minutos por servidor y texto. Dos búsquedas iguales a la vez lanzan un solo proceso. Si YouTube pide verificación de bot, espera 8 s y reintenta una vez. El error de YouTube se guarda para `describeError` (S-13). No hay respaldo en otra plataforma.
8. Resultados para `/search` (`buscar`): `ytsearch<n>` con `--flat-playlist`. Solo entradas con id válido. Innertube se retira (S-15).
9. Búsqueda de listas (`buscar_listas`): la página de resultados de YouTube con el filtro `sp=EgIQAw%3D%3D` (solo listas) y `--flat-playlist`.
10. Relacionados (`relacionados`): `watch?v=<id>&list=RD<id>` con `--flat-playlist --playlist-end 15`. Filtro: id válido, distinto de la canción actual, y duración entre 60 y 900 s cuando hay duración.
11. Precarga: `prefetch` obtiene la información de la siguiente canción con prioridad baja, sin descargar audio. `prefetchQuery` hace lo mismo con una búsqueda de texto (Spotify).
12. Spotify: cada tema se reproduce buscando su texto en YouTube (`createSearchQuery`). Los metadatos vienen del plugin de Spotify. Si hay claves de API, se usan; si no, scraping público. Una lista de Spotify carga solo las primeras 100 canciones (aviso del preflight).
13. Enlaces directos (`RelayedDirectLinkPlugin`): el enlace de audio pasa por el relay (S-15). Así ffmpeg no abre conexiones de red por su cuenta.
14. Otros sitios (`GenericPlugin`): cualquier enlace http o https que yt-dlp soporte. Es el último plugin de la lista. Con listas usa `--flat-playlist --playlist-end <MAX>` y `-f bestaudio/best`. No tiene relacionados. Si `age_limit` es 18 o más, marca edad restringida.
15. Caché de información de flujo: la clave es el proxy más la URL. El TTL es el parámetro `expire` de la URL de audio, menos 30 minutos, con límites de 60 s y 4 horas. Sin `expire`, 1 hora.

## Tests

- Unitarios de `test/plugins.test.js`:
  - `parseYouTubeUrl` con las formas comunes de URL.
  - `youtubeInfo` construye URL y miniatura estables, y maneja directos y edad restringida.
  - `splitYouTubeTitle` separa artista y canción.
  - Las letras se marcan y se limpian.
  - `streamTtl` sigue la caducidad de googlevideo, con margen.
  - Un video se resuelve con una sola llamada a yt-dlp.
  - Las listas se resuelven planas y saltan privados o eliminados.
  - `searchSong` guarda en caché, y las llamadas concurrentes comparten un proceso.
  - `searchSong` informa el error de YouTube, sin respaldo.
  - Sin resultados, se informa el error de YouTube.
  - Un flujo de YouTube que falla con 403 informa el error.
  - Información de flujo: una sola extracción por canción. La precarga usa prioridad baja y no descarga.
  - Los relacionados excluyen la canción actual, los shorts y los videos muy largos.
  - `GenericPlugin` maneja un medio único y listas de otros sitios.
- Propios de Rust: el filtro de duración y de id, el orden de los hosts y la caché compartida.
- Paridad con Node: `scripts/titulos-paridad-ref.js` guarda la salida de `splitYouTubeTitle`, `parseYouTubeUrl` y `streamTtl` para cada caso de `titulos-casos.json`. El test de Rust compara.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Para cada caso de `titulos-casos.json`, Rust y Node dan el mismo artista, título, id y tipo de URL.
- Dos búsquedas iguales a la vez lanzan un solo proceso de yt-dlp (medido con el binario falso de S-13).
- Un error de verificación de bot se reintenta una vez. Si vuelve a fallar, informa el error.
- Ningún archivo pasa de 250 líneas.

## Listas guardadas (`resolvePlaylist`)

Lo usan `/new-playlist` y `/playlists reproducir` (S-25). Origen: `src/music/playlists.js` (`resolvePlaylist`, `detectSource`, `deriveName`, `snapshotTrack`).

1. Entrada: la URL y un límite de temas. El límite es `maxSize`, o `maxPlaylistSize` de la config, o 200 si no hay ninguno. Una URL vacía falla con `EMPTY_URL`. Una URL no soportada falla en `assertSupportedUrl`.
2. Spotify se lee con su API (ver Decisiones). YouTube y otros sitios se leen con yt-dlp (S-13), sin innertube (S-15). Las listas de YouTube se leen en modo de lista plana.
3. La resolución no une el bot a voz.
4. La lista devuelve como máximo el límite de temas. Si la original tiene más, `truncated` es verdadero.
5. Una pista suelta devuelve `kind: track`, con un tema y `truncated` falso.
6. Salida: `{kind, name, tracks, truncated, source}`. `source` es `spotify`, `youtube` o `generic`, según la URL.
7. Cada tema se guarda con `snapshotTrack`, que usa `serializeTrack` de `src/api/contracts.js`. Ese formato se porta con la API (M6).
8. El nombre final lo fija `/new-playlist` con `deriveName(url, nombre dado o nombre resuelto)`, con máximo 60 caracteres. Si la URL no se puede leer, el nombre es "Nueva playlist".

## Decisiones

- Spotify: se usa la API de Spotify con credenciales de cliente (`.env`). El scraping no se porta. Sin claves, los enlaces de Spotify no funcionan, y el arranque lo informa (S-01, regla j). Cambia el comportamiento de Node.
- Enlaces directos y otros sitios: pasan por el runner de S-13. Cookies, proxy y token llegan igual que en las búsquedas. S-15 mantiene la prueba de que songbird recibe esa salida.
- Listas de Spotify: se recortan a 100 canciones, como en Node.

## Preguntas abiertas

- Ninguna en esta spec.
