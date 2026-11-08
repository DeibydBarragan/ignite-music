# Spec S-24: Panel en Components V2

- Estado: borrador
- Fase: F5
- Módulo: M5
- Origen en Node (solo lectura): `src/music/panel.js` (`sendPanel`, `refreshPanel`, `endPanel`, `replacePanel`, `isCurrentPanel`, `adoptPanel`, `startRefresh`, `snapshot`, `panelPayload`, `PANEL_DEBOUNCE_MS`), `src/ui/views.js` (`ID`, `playerPanel`, `endedPanel`, `notice`, `baseView`), `src/handlers/interactions.js` (`handlePanel`, `updatePanel`, `volumeModal`, `handleVolumeModal`, `asUpdate`), `src/handlers/favorite.js` (`handleFavorite`), `src/commands/nowplaying.js`. Pruebas: `test/views.test.js` (panel) y `test/manager.test.js` (el panel se envía una vez).

## Objetivo

Enviar, editar y cerrar el panel del reproductor. Es un mensaje con Components V2 y botones, en el canal de texto de la cola. Atiende sus botones, su menú de filtros y su modal de volumen.

Los `custom_id` son los mismos que en Node (`m:...`), para que la prueba de paridad compare los mensajes tal cual.

No incluye las vistas de cola, letras, búsqueda, ayuda y playlists (S-25), ni los permisos (S-23). Los favoritos dependen de la tabla `user_favorites` en Supabase (S-30).

## Archivos

- Crear: `rust/crates/bot/src/panel/mod.rs`: funciones del ciclo de vida (`enviar_panel`, `refrescar_panel`, `cerrar_panel`, `reemplazar_panel`, `adoptar_panel`, `es_panel_actual`, `iniciar_refresco`, `detener_refresco`).
- Crear: `rust/crates/bot/src/panel/servidor.rs`: tarea por servidor que recibe órdenes por un canal `mpsc`. Guarda el mensaje, el temporizador de 750 ms y el refresco periódico.
- Crear: `rust/crates/bot/src/panel/instantanea.rs`: `snapshot`, los datos de la cola que usa la vista.
- Crear: `rust/crates/bot/src/panel/interaccion.rs`: botones `m:*`, menú de filtros, modal `m:volmodal` y `/nowplaying`.
- Crear: `rust/crates/bot/src/ui/componentes.rs`: IDs, filas, botones, texto, separador y sección. S-25 también lo usa.
- Crear: `rust/crates/bot/src/ui/panel.rs`: `vista_reproductor` y `vista_terminada`.
- Crear: `rust/crates/bot/tests/panel.rs` y `rust/crates/bot/tests/fixtures/panel-node.json`.
- No tocar: `src/music/panel.js`, `src/ui/views.js`, `src/handlers/interactions.js`, `src/handlers/favorite.js`, `src/commands/nowplaying.js`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

- Entrada de la vista: instantánea de la cola. Incluye la canción actual, si está pausada, la posición en segundos, el volumen (0 a 100), el bucle (0 apagado, 1 canción, 2 cola), si autoplay está activo, el filtro activo (o nada), las próximas canciones y su duración total. Incluye también el traductor del servidor (S-03).
- Salida: mensaje V2 con un contenedor y la bandera de Components V2 (`1 << 15`). `allowed_mentions` con `parse: []`. Es efímero solo cuando la ruta que responde lo pide.
- Órdenes del ciclo de vida, por servidor: `enviar`, `refrescar` (con o sin inmediato), `cerrar` (título, descripción opcional, canción opcional), `reemplazar` (mensaje), `adoptar` (mensaje) y `es_actual` (ID del mensaje).
- Errores de Discord que olvidan el panel: 10008 (mensaje desconocido), 10003 (canal desconocido), 50001 (sin acceso) y 50013 (sin permisos). Cualquier otro error se registra como aviso, y la cola sigue.

## Comportamiento

1. Orden: cada servidor tiene su tarea (`servidor.rs`). Las órdenes se procesan una a una, en el orden en que llegan. Si una orden falla, se registra `Panel update failed in <servidor>: <mensaje>` y la siguiente orden sigue. Esto reemplaza la cadena de promesas `enqueue` de Node.
2. `enviar`:
   1. No hace nada si no hay canal de texto (S-23), si la cola está parada o si no tiene canciones.
   2. Si el panel anterior es el último mensaje del canal, lo edita.
   3. Si no, borra el anterior sin esperar el resultado, y envía uno nuevo.
   4. Si la edición falla con un código de la lista de olvido, olvida el panel y envía uno nuevo.
3. `refrescar` sin inmediato: reinicia un temporizador de 750 ms por servidor. Al vencer, edita el panel con la vista actual. Cuatro llamadas dentro de 750 ms producen una sola edición.
   `refrescar` con inmediato: edita ya, sin temporizador.
   Sin panel, cola parada o cola vacía: no hace nada.
4. `cerrar`: cancela el temporizador y el refresco periódico. Edita el panel con la vista terminada, sin botones, y olvida el panel. Un refresco posterior no hace nada.
5. `reemplazar` (lo usa `/nowplaying`): cancela el temporizador. El mensaje nuevo pasa a ser el panel. El anterior se borra si es otro mensaje. Los errores se ignoran.
6. `adoptar` (tras editar desde un botón): cancela el temporizador y guarda el mensaje como panel.
7. `es_panel_actual`: compara el ID del mensaje de la interacción con el ID del panel.
8. Refresco periódico: cada `PANEL_REFRESH` segundos (S-01). Con 0, está apagado. En cada tic, si la cola ya no existe o está parada, el refresco se detiene. Si la cola no está pausada, refresca de inmediato.
9. Instantánea: la canción es la primera de la cola. Las próximas son el resto. La duración de las próximas es la suma de sus duraciones, y 0 si falta alguna. El filtro es el primero de los filtros activos, o nada.
10. Vista del reproductor (`vista_reproductor`):
    1. Contenedor con color de acento `EMBED_COLOR` (por defecto `#5865F2`). Un contenedor admite hasta 10 componentes. Es el límite de Discord que refleja `Container` en twilight-model 0.17.0.
    2. Cabecera: línea `-#` con el estado (`commands.nowplaying.title` o `commands.nowplaying.status_paused`). Después, el título con enlace (máximo 90 caracteres) y una línea con el autor (máximo 60) y la duración o "En directo". Si la URL de la miniatura es http o https, va como sección con miniatura.
    3. Barra de progreso si no es directo y la duración es mayor que 0: tiempo actual, barra de 16 celdas y duración total.
    4. Línea de detalles separada por " · ": quién la pidió (mención, o "añadida por autoplay"), volumen, bucle, autoplay y filtro (si hay).
    5. Línea de la siguiente canción: enlace (máximo 60), quién la pidió, y un pie con el número de canciones y la duración total. Sin próximas, el pie dice que no hay canciones próximas.
    6. Separador.
    7. Fila 1: Anterior (`m:prev`), Pausar o Reanudar (`m:pause`, primario), Saltar (`m:skip`), Parar (`m:stop`, peligro) y Volumen (`m:vol`).
    8. Fila 2: Mezclar (`m:shuffle`, desactivado con menos de 2 próximas), Bucle (`m:loop`, verde si no está apagado), Autoplay (`m:auto`, verde si está encendido), Cola (`m:queue`) y Favorito (`m:fav`).
    9. Fila 3: menú de filtros (`m:filter`). Su texto es "filtro: <actual>". Opciones: Ninguno (`none`) y los 8 filtros, con el nombre como valor.
    10. Fila 4: Letras (`m:lyrics`).
    11. Ningún botón lleva emoji.
11. Vista terminada (`vista_terminada`): contenedor con título, descripción opcional y enlace opcional. No tiene botones.
12. Botones `m:*` (`panel/interaccion.rs`):
    1. Si no hay cola, si no tiene canciones o si el mensaje no es el panel actual, actualiza el mensaje a la vista terminada con `musicmanager.playback_ended`.
    2. `m:queue` responde con la vista de cola (S-25). `m:lyrics` muestra la letra de la canción actual (S-25). `m:fav` alterna el favorito (regla 14). Estos tres no pasan por el control de voz.
    3. Los demás pasan por `revisar_voz` (S-23), sin unirse. Si falla, avisa.
    4. `m:pause`, `m:loop`, `m:auto`, `m:shuffle` y `m:filter` ejecutan su acción (S-17). Si falla, avisa con el mensaje de la acción. Si sale bien, actualiza el mensaje con la vista nueva y lo adopta. No refresca otra vez.
    5. `m:vol` abre el modal `m:volmodal`: un campo `volume`, obligatorio, de 1 a 3 caracteres, con el volumen actual. El título y la etiqueta vienen de i18n, recortados a 45 caracteres.
    6. `m:skip` y `m:prev` difieren la actualización y ejecutan la acción. Si falla, envían un seguimiento con el aviso. Si sale bien, no envían nada más. El cambio llega por los eventos de la cola.
    7. `m:stop` difiere la actualización y ejecuta `stop` (S-17), que cierra el panel y sale del canal.
13. Modal `m:volmodal`: comprueba voz (S-23). Si el texto tiene solo dígitos, lo convierte en número. Si no, usa NaN, que `setVolume` rechaza. Si el modal sale de un mensaje y ese mensaje es el panel actual, actualiza el panel. Si no, llama a `changed` y avisa con el resultado.
14. Favorito (`m:fav`): busca la canción en `user_favorites` del usuario, por `song_id` o por URL. Si existe, la borra y avisa de que se eliminó. Si no existe, la inserta con título, artista (por defecto "Desconocido"), portada, URL, duración y fuente (por defecto "youtube") y avisa de que se guardó. Si falla, avisa con un mensaje fijo. Sin Supabase, avisa de que la base de datos no está disponible.
15. `asUpdate`: en las respuestas de tipo actualización se quita la bandera efímera. Discord no permite cambiarla en un mensaje que ya existe.
16. `/nowplaying`: si no hay cola con canciones, avisa con `no_track`. Si la hay, responde con el panel y lo registra con `reemplazar`, usando el mensaje de la respuesta.

## Tests

- Debounce: cinco refrescos en 500 ms con reloj simulado producen una edición a los 750 ms.
- Refresco inmediato: edita sin esperar.
- `enviar`: la primera vez envía. La segunda edita. Si el último mensaje del canal cambió, borra el anterior y envía uno nuevo (como `test/manager.test.js`).
- Errores 10008 y 50013: olvida el panel y reenvía. Otro error: se registra y la cola sigue.
- `cerrar`: edita a la vista terminada. Un refresco posterior no edita nada.
- Refresco periódico: se detiene al parar la cola y no refresca si está pausada.
- Forma del mensaje: bandera V2, contenedor con color, 10 componentes o menos, `custom_id` de 100 caracteres o menos, etiquetas sin emoji. "Pause" o "Resume", y "Loop: Off" o "Loop: Queue", en inglés, como en `test/views.test.js`.
- Paridad: el JSON de `vista_reproductor` con la misma entrada que `playerPanel` de Node se compara con `panel-node.json`. Casos: reproduciendo; en pausa con bucle de cola y autoplay; en directo; duración 0; nombre de 500 caracteres; sin próximas; con filtro.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Al empezar una canción aparece el panel en el canal donde se pidió.
- Cada botón (pausa, saltar, anterior, parar, volumen, mezclar, bucle, autoplay, cola, letras, favorito y filtro) hace su acción y actualiza el panel.
- Con 10 cambios seguidos, el panel se edita como máximo una vez cada 750 ms. Se comprueba en el log de ediciones del servidor de prueba.
- Un panel viejo muestra la vista terminada, sin botones.
- `panel-node.json` y el JSON de Rust coinciden en los casos de la lista.
- `cargo test` pasa.

## Preguntas abiertas

1. Componentes V2 en twilight. Confirmado en la documentación de `twilight-model` 0.17.0: `Container` (con `accent_color`), `TextDisplay`, `Section`, `Thumbnail`, `Separator`, `ActionRow` y `SelectMenu`. Falta comprobar en el servidor de prueba que Discord acepta el mensaje.
2. Tarea por servidor. Propuesta: una tarea con canal `mpsc` por servidor, en lugar de la cadena de promesas de Node. Así las órdenes de un servidor no se pisan, y no hacen falta bloqueos compartidos.
3. Texto fijo en `favorite.js`, en español. Propuesta: mantenerlo en F5, por paridad, y pasarlo a `languages/*.json` con F4 (e).
4. Límite de texto. Node recorta a 3900 caracteres en `notice`. El límite de un TextDisplay se verifica en la documentación de Discord antes de cerrar esta spec.
