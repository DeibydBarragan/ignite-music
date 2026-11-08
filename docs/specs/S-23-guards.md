# Spec S-23: Guards y permisos

- Estado: borrador
- Fase: F5
- Módulo: M5
- Origen en Node (solo lectura): `src/handlers/guards.js` (`checkVoice`, `panelChannel`, `respond`, `say`, `IGNORED_CODES`), versión de HEAD. Los cambios sin commit de ese archivo, `src/core/dev-mode.js`, `src/config.js` y `test/dev-mode.test.js` se tratan en Preguntas abiertas.

## Objetivo

Decidir si un usuario puede mandar sobre la música: que esté en voz, en el mismo canal que el bot, con permisos y sin canal lleno. Decidir también a qué canal de texto va el panel. Y responder a interacciones que quizá ya vencieron sin fallar.

No incluye los comandos (S-22) ni el panel (S-24).

## Archivos

- Crear: `rust/crates/bot/src/permisos.rs`: `revisar_voz` y `canal_de_panel`.
- Crear: `rust/crates/bot/src/respuesta.rs`: `responder` y `avisar`.
- Crear: `rust/crates/bot/tests/permisos.rs`.
- No tocar: `src/handlers/guards.js`, `src/core/dev-mode.js`, `src/config.js` y `test/dev-mode.test.js`. Son cambios del usuario sin commit.

Cada archivo tiene como máximo 250 líneas.

## Contrato

Entrada de `revisar_voz`:

- `canal_usuario`: canal de voz del usuario según la caché (S-21), o nada.
- `canal_bot`: canal de voz del bot, o nada.
- `cola`: cola del servidor (S-17), con sus canciones.
- `permisos_bot`: permisos del bot en el canal, o nada.
- Opciones: `requiere_cola` (por defecto, sí) y `unirse` (por defecto, no).

Salida: `Ok { canal, cola }` o `Err(clave)`. La clave es de i18n (S-03). Se devuelve el primer error de esta lista, en este orden:

1. `commands.play.voice_channel_required`: el usuario no está en voz.
2. `commands.play.same_channel_required`: el bot está en voz en otro canal.
3. `commands.nowplaying.no_track`: `requiere_cola` y no hay cola, o la cola está vacía.
4. `commands.play.no_permissions`: `unirse`, el bot no está en voz y le faltan Ver canal, Conectar o Hablar en el canal.
5. `ui.voice_full`: `unirse`, el bot no está en voz, el canal está lleno y el bot no tiene Mover miembros.

Entrada de `canal_de_panel`: canal de la interacción y permisos del bot en él. Salida: canal o nada.

Entrada de `responder`: interacción y respuesta. Salida: respuesta enviada o nada. Detalle en Comportamiento.

## Comportamiento

1. `revisar_voz` evalúa las reglas en el orden del Contrato y devuelve el primer error.
2. La regla 2 solo aplica si el bot ya está en voz. Si no lo está, el usuario puede estar en cualquier canal.
3. Las reglas 4 y 5 solo aplican si `unirse` está activo y el bot no está en voz. Si el bot ya está en voz, no se revisan permisos de unión.
4. Lleno: el canal tiene límite de usuarios y lo alcanza. Es el `channel.full` de discord.js.
5. `canal_de_panel`:
   1. Devuelve nada si el canal no es de un tipo con texto que DisTube 5.2.3 acepte (`isTextChannelInstance` en `node_modules/distube/dist/index.js`, línea 1259). Por ejemplo, una categoría no sirve.
   2. Devuelve nada si el bot no tiene Ver canal y Enviar mensajes.
   3. En hilos, además necesita Enviar mensajes en hilos.
   4. Si devuelve nada, la cola sigue sin panel. La reproducción no se cancela. Node comprueba `textChannel` antes de enviar en `playback.js` y `events.js`.
6. `responder`:
   1. Si la interacción está diferida y aún no tiene respuesta, edita la respuesta diferida.
   2. Si ya tiene respuesta, envía un seguimiento.
   3. Si no, envía una respuesta nueva.
   4. Los códigos 10062 (interacción desconocida) y 40060 (interacción ya reconocida) no son error. En ese caso no devuelve nada.
   5. Cualquier otro error sube al llamador.
   6. Al editar una respuesta diferida, solo se conserva la bandera de Components V2. La bandera efímera la fijó el diferido.
7. `avisar(interaccion, mensaje, opciones)` es `responder` con un aviso, que es un contenedor de texto. Es efímero por defecto.
8. Estado de voz: el objeto `member` de la interacción no trae el canal de voz. El canal del usuario y el del bot salen de la caché (S-21), que se alimenta de `VoiceStateUpdate`.
9. Permisos: para el canal de la interacción se usa `app_permissions`, que Discord envía en la interacción. Para otros canales, como el del usuario con `unirse`, los permisos se calculan con la caché (roles y sobrescrituras).

## Tests

- Tabla de `revisar_voz` con 9 casos: sin voz; bot en otro canal; bot en el mismo canal; sin cola con `requiere_cola`; con cola; `unirse` con permisos; `unirse` sin permisos; canal lleno sin Mover miembros; canal lleno con Mover miembros.
- Paridad con Node: los mismos casos contra `checkVoice` de HEAD, con el mismo estado. Se comparan las claves de error.
- `canal_de_panel`: canal de texto, hilo, canal de voz con texto, categoría (no), sin permisos (no), hilo sin permiso de hilos (no).
- `responder`: sin respuesta, diferida, con respuesta. 10062 y 40060 no son error. 50013 sube.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Un usuario fuera de voz recibe el aviso de `voice_channel_required` en su idioma.
- Un comando de voz desde otro canal recibe `same_channel_required`.
- Una interacción vencida no deja errores en el log.
- En un canal donde el bot no puede escribir, la reproducción sigue sin panel.
- `cargo test` pasa con la tabla de casos.

## Preguntas abiertas

1. Modo dev (sin commit). El archivo de trabajo de `guards.js` permite que los usuarios que cumplen `isDevUser` usen comandos de voz sin estar en un canal. También salta la regla de mismo canal. Además, elige un canal de voz disponible con `findAvailableVoiceChannel`. `test/dev-mode.test.js` ya prueba este caso. Propuesta: no portarlo en F5. Se decide cuando el usuario haga commit de ese cambio.
2. Riesgo de seguridad. El modo dev salta la regla de mismo canal. En el archivo de trabajo de `config.js`, la lista sale de `DEV_USER_IDS`. Si esa variable no existe, usa `NEXT_PUBLIC_DEV_USER_IDS`, y si tampoco existe, un ID fijo en el código (`472144161357824000`). Una variable con prefijo `NEXT_PUBLIC_` llega al navegador si el dashboard la usa. Propuesta: si se porta, sin ID por defecto y sin variable pública.
3. Caché de voz y permisos (S-21). Propuesta: `twilight-cache-inmemory` con `permission-calculator`. Si no se aprueba, hace falta una caché propia de voz y roles.
