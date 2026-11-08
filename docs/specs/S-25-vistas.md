# Spec S-25: Vistas de cola, letras, búsqueda, ayuda y playlists

- Estado: borrador
- Fase: F5
- Módulo: M5
- Origen en Node (solo lectura): `src/ui/auxiliary-views.js` (`queueView`, `lyricsView`, `searchView`, `playlistsView`, `helpView`, `languageView`, `splitLyrics`, `pager`), `src/commands/queue.js`, `search.js`, `lyrics.js`, `help.js`, `playlists.js`, `new-playlist.js` y `language.js`, `src/handlers/interactions.js` (`handleQueuePage`, `handleLyricsPage`, `handleSearchPick`, `handleLanguage`), `src/music/lyrics.js` (`findLyrics`, `cleanTitle`, `pickBest`), `src/music/playlists.js` (`resolvePlaylist`, `fetchUserPlaylists`, `upsertUserPlaylist`, `deriveName`, `detectSource`; la tabla de Supabase está en S-30), `src/core/format.js` (`formatDuration`, `parseTime`, `progressBar`, `truncate`, `escapeMarkdown`, `safeUrl`, `link`, `linkLabel`, `chunk`). Pruebas: `test/views.test.js`, `test/core.test.js` (formato), `test/misc.test.js` (`cleanTitle`, `pickBest`).

## Objetivo

Portar las vistas que no son el panel: cola, letras, búsqueda, ayuda, idioma y playlists. Incluye las funciones de formato que esas vistas usan.

No incluye el panel (S-24), los permisos (S-23) ni los comandos de reproducción (S-22).

## Archivos

- Crear: `rust/crates/bot/src/ui/cola.rs`, `ui/letras.rs`, `ui/busqueda.rs`, `ui/ayuda.rs` (ayuda e idioma) y `ui/listas.rs` (playlists).
- Crear: `rust/crates/bot/src/ui/interaccion.rs`: `q:<página>`, `ly:<token>:<página>`, `s:<token>` y `lang:set`.
- Crear: `rust/crates/bot/src/comandos/ver_cola.rs`, `letras.rs`, `buscar.rs`, `ayuda.rs`, `idioma.rs` y `listas.rs` (`playlists` y `new-playlist`).
- Crear: `rust/crates/medios/src/letras.rs`: `buscar_letras`, `limpiar_titulo` y `elegir_mejor`, con la fuente LRCLIB. Crate `medios` (M3), aprobado en F4.
- Crear: `rust/crates/bot/src/formato.rs`: funciones de formato. Puede pasar al crate `nucleo` si se aprueba.
- Crear: `rust/crates/bot/tests/vistas.rs`, `tests/formato.rs`, `tests/letras.rs`, `tests/fixtures/vistas-node.json` y `tests/fixtures/formato-node.json`.
- No tocar: `src/ui/auxiliary-views.js`, `src/commands/`, `src/handlers/interactions.js`, `src/music/lyrics.js`, `src/music/playlists.js` y `src/core/format.js`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

Cola (`/queue` y el botón de cola):

- Entrada: canción actual, próximas y página (base 0).
- Salida: vista V2 efímera.

Letras:

- Entrada: `{titulo, artista, letra, fuente}`, token y página.
- Salida: vista V2 efímera con una página de hasta 2800 caracteres.
- `buscar_letras(cancion)`: devuelve la letra, o nada si no hay. Un error de red o un código HTTP distinto de 404 es error.

Búsqueda (`/search`):

- Entrada: consulta (máximo 200 caracteres), hasta 10 resultados y token.
- Salida: vista V2 efímera con la lista y un menú de selección de hasta 25 opciones.

Ayuda (`/help`):

- Entrada: comandos (nombre, ID si existe y descripción localizada) y estadísticas (servidores, activos, tiempo activo en milisegundos y latencia).
- Salida: vista V2 efímera, con enlaces solo si la URL es http o https.

Idioma (`/language`):

- Entrada: los 23 idiomas y el idioma actual.
- Salida: vista V2 efímera con un menú de selección.

Playlists (`/playlists`):

- Entrada: playlists del usuario (Supabase, S-30), filtro y página de 10.
- Salida: vista V2 efímera con la lista.

Nueva playlist (`/new-playlist`):

- Entrada: URL (máximo 1000 caracteres) y nombre opcional (máximo 60).
- Salida: aviso de que se guardó o se actualizó la playlist.

Formato: ver Comportamiento, regla 8.

## Comportamiento

Los textos de las vistas salen de i18n (S-03). Los textos entre comillas de esta spec son la referencia en español de la clave indicada.

1. Cola (`queueView`):
   1. Diez canciones por página. La página se ajusta al rango de 0 a páginas menos 1.
   2. Cabecera: texto de `buttonhandler.now_playing`, con el enlace, la duración (o "En directo" si aplica) y quién la pidió.
   3. Lista numerada `NN.` con enlace (máximo 50), duración y quién la pidió (mención, o *autoplay*).
   4. Pie: número de canciones, duración total y "página X de Y".
   5. Si hay más de una página, botones "Atrás" (`q:<n-1>`) y "Siguiente" (`q:<n+1>`), desactivados en los extremos.
   6. `/queue` sin canciones avisa con `commands.nowplaying.no_track`. El botón `q:<n>` sin cola hace lo mismo y actualiza el mensaje.
2. Letras:
   1. `dividir_letras` parte el texto por líneas en blanco. Junta párrafos hasta 2800 caracteres. Un párrafo más largo se corta en trozos de 2800. Sin texto, devuelve una página vacía.
   2. La vista tiene un título "Letras", un subtítulo "título · artista" (escapado, máximo 80 y 60), un separador, el texto de la página con los `#` y los `-` del principio escapados, y un pie "fuente · página X de Y". Si hay más de una página, añade los botones `ly:<token>:<n>`.
   3. `/lyrics [canción]` responde de forma diferida y efímera. Sin canción y sin cola, avisa con `no_track`. Con cola, usa la canción actual. Si falla la búsqueda, avisa con `buttonhandler.lyrics_error`. Si no hay letra, avisa con `buttonhandler.no_lyrics_found` y el nombre. El token tiene 12 caracteres hexadecimales, dura 30 minutos y hay como máximo 500.
   4. El botón `ly:<token>:<n>` con un token que no existe avisa con `buttonhandler.lyrics_unavailable`.
   5. `buscar_letras` consulta `https://lrclib.net/api`:
      - Limpia el título: quita lo que va entre paréntesis, corchetes o 【】, y las palabras official, music, lyric, lyrics, audio, video, visualizer, hd, 4k, mv y remastered. Quita también lo que sigue a "ft" o "feat". Si el título tiene un guion con espacios alrededor, la parte izquierda es el artista. El artista del canal pierde el sufijo " - Topic" y "VEVO".
      - Busca por título y artista. Si no hay resultados, busca por "artista título".
      - Elige la pista con letra plana, que no sea instrumental y cuya duración sea la más cercana a la de la canción.
      - Caché de 6 horas, con máximo de 500 entradas. Tiempo límite de 8 s. Una respuesta 404 da nada. Otro código HTTP es error.
3. Búsqueda (`/search`):
   1. `revisar_voz` con unirse y sin requerir cola (S-23). Respuesta diferida y efímera.
   2. Consulta vacía: aviso fijo "Debes ingresar un término de búsqueda." (ver Preguntas abiertas).
   3. `manager.youtube.search(consulta, 10, servidor)` (S-14). Un error pasa por `explainError`. Sin resultados, avisa con `commands.search.no_results`.
   4. Guarda `{usuario, resultados}` con un token de 12 caracteres hexadecimales, durante 10 minutos, con máximo de 500 entradas.
   5. Vista: hasta 10 resultados, cada uno con `N.` y enlace, y una línea "-# autor · duración". El menú `s:<token>` tiene una opción por resultado: etiqueta "N. nombre" (máximo 100) y descripción "autor · duración" (máximo 100). El valor es el índice.
   6. Al elegir (`s:<token>`): si no hay entrada, si expiró, si la eligió otro usuario o si el índice no existe, actualiza el mensaje con `search_expired`. Si es válida, difiere la actualización y llama a `reproducir_desde_interaccion(url)` (S-22). La respuesta reemplaza el mensaje del menú.
4. Ayuda (`/help`):
   1. Orden: categoría (`playback`, `queue`, `settings`) y después nombre.
   2. Descripción: la del idioma del servidor (S-03). Si no hay traducción, la del comando.
   3. Nombre: mención `</nombre:ID>` si hay ID (S-22). Si no, `` `/nombre` ``.
   4. Estadísticas: servidores (la caché del único shard), activos (colas de reproducción activas, S-17), tiempo activo y latencia (`shard.latency().average()`, en milisegundos, redondeada y nunca negativa).
   5. `formatUptime(ms)`: minutos = piso(ms / 60000). Días = piso(minutos / 1440). Horas = piso((minutos mod 1440) / 60). Muestra "Xd" y "Yh" solo si son mayores que 0, y siempre "Zm". Ejemplos: 0 ms da "0m"; 61 minutos dan "1h 1m"; 1500 minutos dan "1d 1h 0m".
   6. Enlaces de soporte, sitio web e invitación. Solo se muestran si la URL es http o https.
5. Idioma (`/language`):
   1. Muestra "Idioma actual: **nombre**" y un menú `lang:set` con los idiomas, hasta 25.
   2. Al elegir: si quien elige no tiene Gestionar servidor, avisa con `permission_required_button`. Si lo tiene, guarda el idioma (S-03), refresca el panel (S-24) y edita el mensaje con el aviso `changed_desc`, con el nombre del idioma.
6. Playlists (`/playlists`):
   1. Respuesta diferida y efímera. Lee las playlists del usuario (S-30). Si falla, avisa con un mensaje fijo: "No pude cargar tus playlists ahora mismo. Inténtalo de nuevo en un momento."
   2. Con `reproducir`, busca en este orden: URL exacta, nombre exacto (sin distinguir mayúsculas) y nombre que contenga el texto. Si no hay resultado, lista hasta 5 nombres, o avisa de que no hay playlists y sugiere `/new-playlist`. Si hay resultado, `revisar_voz` con unirse y después `manager.play(url)` (S-17). El mensaje cuenta los temas añadidos, con una nota si se recortó, o nombra la canción. Un error pasa por `explainError`. La opción `page` se ignora.
   3. Sin `reproducir`, filtra por nombre o por fuente (contiene, sin distinguir mayúsculas). La vista `playlistsView` muestra 10 por página: `NN.` con el nombre (máximo 60) y "-# fuente · N temas · duración". Sin playlists, usa `ui.playlists_empty`. El título incluye el total y el filtro, si hay uno.
7. Nueva playlist (`/new-playlist`):
   1. Respuesta diferida y efímera. Una URL que no empieza por http o https avisa. Una URL de SoundCloud avisa de que ya no está soportada.
   2. `resolvePlaylist` resuelve la playlist sin unirse a voz. Un error pasa por `explainError`. Sin temas, avisa.
   3. Nombre: `deriveName(url, nombre o nombre resuelto)`, con máximo 60.
   4. `upsertUserPlaylist` guarda por usuario y URL. Si es nueva, avisa de que se guardó. Si ya existía, de que se actualizó. El aviso indica el número de temas, con una nota si se recortó al máximo de `maxPlaylistSize`.
   5. Un error al guardar avisa con un mensaje fijo.
8. Formato (`formato.rs`, con las mismas reglas que `core/format.js`):
   - `formatDuration(s)`: "m:ss", o "h:mm:ss" si hay horas. Los negativos y los NaN valen 0.
   - `parseTime(texto)`: acepta "90", "1:30" y "1:02:03". Devuelve nada si no cumple el formato, o si minutos o segundos son 60 o más.
   - `progressBar(actual, total)`: 16 celdas, con "━" y un "●" en la posición actual. Si el total es 0, devuelve 16 guiones "─".
   - `truncate(texto, max)`: si supera `max`, corta a `max - 1` y añade "…".
   - `escapeMarkdown`: antepone una barra invertida a la barra invertida y a los caracteres de formato: asterisco, guion bajo, acento grave, tilde, barra vertical, mayor que, corchetes y paréntesis.
   - `safeUrl`: acepta solo http y https. Cambia `)` por `%29`.
   - `link(titulo, url, max)`: `[etiqueta](url)` si la URL es válida. Si no, devuelve el texto escapado y recortado.
   - `linkLabel`: quita emoji, caracteres invisibles y esquemas de URL. Cambia `[` y `]` por `(` y `)`, y cambia los puntos entre letras por "․" (U+2024). Si no queda ninguna letra ni número, usa "Untitled".
   - `chunk(lista, n)`: `n` debe ser un entero de 1 o más. Si no, falla con un error equivalente a `RangeError`.

## Tests

- `formato-node.json`: los casos de `test/core.test.js` (`formatDuration`, `parseTime`, `progressBar`, `truncate`, `escapeMarkdown`, `link` y `safeUrl`), más `linkLabel` (emoji, corchetes, URL en el título y título vacío) y `chunk` (`RangeError`). Node y Rust deben dar la misma salida.
- `letras`: `cleanTitle` y `pickBest` con los casos de `test/misc.test.js`. `dividir_letras`: párrafo largo, varias páginas y texto vacío.
- `listas`: búsqueda de playlist por URL exacta, por nombre exacto, por nombre que contiene el texto, y sin resultado.
- `ayuda`: `formatUptime` con los tres ejemplos de Comportamiento 4.5. Orden de comandos por categoría.
- `busqueda`: entrada inexistente, expirada, de otro usuario o índice fuera de rango dan `search_expired`.
- `idioma`: sin Gestionar servidor da `permission_required_button`.
- `vistas-node.json`: cola (con paginado y página fuera de rango), letras, búsqueda, ayuda, idioma, playlists, aviso y vista terminada, con entradas fijas en inglés. Se compara la forma y los textos.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- `/queue` y sus botones cambian de página sin salirse del rango.
- `/lyrics` muestra la letra por páginas. El botón de letras del panel muestra la misma letra.
- `/search` lista 10 resultados. Elegir uno reproduce la canción.
- `/help` lista los comandos con menciones clicables y muestra las estadísticas.
- `/language` cambia el idioma del servidor y refresca el panel.
- `/playlists` y `/new-playlist` funcionan con Supabase (S-30).
- Los JSON de vistas y de formato coinciden con Node en los casos de la lista.
- Ningún archivo pasa de 250 líneas.

## Decisiones

1. Lugar de `formato` y de `letras`: `letras.rs` va en el crate `medios` (M3), aprobado en F4. `formato.rs` queda en `bot` hasta aprobar `nucleo`.
2. `resolvePlaylist`: se añade a S-14 como sección "Listas guardadas (`resolvePlaylist`)".
3. Textos fijos en español (`search.js`, `playlists.js`, `new-playlist.js`, incluido el mensaje que dice "Vela con", y `favorite.js`): se portan igual, por paridad. Pasarlos a `languages/*.json` es un cambio aparte, con la regla de F4 (e).
4. "Activos" en `/help`: se cuentan las colas del reproductor de F4 (S-17).
5. `/search` exige estar en voz antes de buscar: se mantiene, por paridad.

## Preguntas abiertas

1. Aprobar el crate `nucleo` (ver S-01). Sin esa aprobación, `formato.rs` queda en `bot`.
2. Límite de texto en `/help` y `/queue`. Node no recorta el texto de la ayuda. Con 25 comandos y descripciones largas, puede superar el límite de un TextDisplay. Se verifica en la documentación de Discord antes de cerrar esta spec.
