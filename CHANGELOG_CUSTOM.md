# Registro de Modificaciones y Mejoras (Custom Changes)

Este documento detalla todas las modificaciones realizadas sobre el repositorio base [umutxyp/MusicBot](https://github.com/umutxyp/MusicBot) para garantizar su correcto funcionamiento en Windows y su futura migración a VPS.

---

## 1. Solución al fallo de DNS en Windows (`EAI_FAIL`)

### Problema
En sistemas Windows (especialmente con Node.js 18+ / 22+), la librería HTTP interna de Discord (`undici`/`discord.js`) realiza consultas DNS con `{ all: true }`, solicitando simultáneamente registros IPv4 (A) e IPv6 (AAAA). En redes domésticas o proveedores sin enrutamiento IPv6 activo, la llamada a la API nativa de Windows `GetAddrInfoW` fallaba con:
```text
Interaction play failed: Error: getaddrinfo EAI_FAIL discord.com (errno: -3004)
```
Esto provocaba que Discord mostrara el error: *"An error occurred during processing!"*.

### Modificación
* **Archivo creado:** [`src/core/dns-fix.js`](file:///c:/Users/Usuario/Documents/antigravity/amazing-franklin/MusicBot/src/core/dns-fix.js)
  * Configura `dns.setDefaultResultOrder('ipv4first')`.
  * Envoltorio (*wrapper*) sobre `dns.lookup` que fuerza `family: 4` para todas las peticiones salientes.
* **Archivos actualizados para cargarlo en el arranque:**
  * [`index.js`](file:///c:/Users/Usuario/Documents/antigravity/amazing-franklin/MusicBot/index.js) (proceso raíz)
  * [`src/setup/launcher.js`](file:///c:/Users/Usuario/Documents/antigravity/amazing-franklin/MusicBot/src/setup/launcher.js) (gestor de shards)
  * [`src/bot.js`](file:///c:/Users/Usuario/Documents/antigravity/amazing-franklin/MusicBot/src/bot.js) (cada shard de Discord)
  * [`scripts/deploy-commands.js`](file:///c:/Users/Usuario/Documents/antigravity/amazing-franklin/MusicBot/scripts/deploy-commands.js) (registro de comandos Slash)

---

## 2. Compatibilidad con Caché de Comandos de Discord (`song:` vs `query:`)

### Problema
El bot anterior (**Lavamusic**) definía la opción del comando `/play` con el nombre `song:`. El cliente de Discord en la app de escritorio o móvil guarda en caché los comandos. Al escribir `/play`, el cliente enviaba el parámetro con la clave `song`.

El código original de **MusicBot** ejecutaba de forma estricta:
```javascript
interaction.options.getString('query', true) // true = lanza excepción si no se encuentra exactamente 'query'
```
Lo que generaba el error interno:
```text
DiscordjsTypeError [CommandInteractionOptionNotFound]: Required option "query" not found.
```

### Modificación
* **Archivos modificados:**
  * [`src/commands/play.js`](file:///c:/Users/Usuario/Documents/antigravity/amazing-franklin/MusicBot/src/commands/play.js)
  * [`src/commands/search.js`](file:///c:/Users/Usuario/Documents/antigravity/amazing-franklin/MusicBot/src/commands/search.js)
* **Código implementado:**
  Se flexibilizó la lectura de parámetros para aceptar tanto `query` como `song`, `name` o el primer argumento disponible:
  ```javascript
  const raw = interaction.options.getString('query') ||
              interaction.options.getString('song') ||
              interaction.options.getString('name') ||
              interaction.options.data[0]?.value;
  ```

---

## 3. Selector de Plataforma Predeterminada para Búsquedas de Texto

### Problema
Por defecto, DisTube asumía siempre YouTube para búsquedas de texto plano (ej. `/play Yan Block`), sin opción en el archivo `.env` para cambiar a SoundCloud.

### Modificación
* **Archivos modificados:**
  * [`src/config.js`](file:///c:/Users/Usuario/Documents/antigravity/amazing-franklin/MusicBot/src/config.js): Se agregó `defaultSearchPlatform: env('DEFAULT_SEARCH_PLATFORM', 'youtube').toLowerCase()`.
  * [`src/music/manager.js`](file:///c:/Users/Usuario/Documents/antigravity/amazing-franklin/MusicBot/src/music/manager.js): Se ordena dinámicamente el array de plugins de DisTube según la variable elegida (`soundcloud` primero o `youtube` primero).
  * [`.env`](file:///c:/Users/Usuario/Documents/antigravity/amazing-franklin/MusicBot/.env): Se documentó y añadió la variable `DEFAULT_SEARCH_PLATFORM=youtube`.

---

## 4. Configuración del Entorno Local

* **Archivo creado:** [`.env`](file:///c:/Users/Usuario/Documents/antigravity/amazing-franklin/MusicBot/.env)
  * Token y Client ID del bot configurados.
  * `GUILD_ID` fijado para sincronización instantánea de comandos en el servidor.
  * `DEFAULT_LANGUAGE=es` para idioma español en embeds y respuestas.
  * `YOUTUBE_FALLBACK=soundcloud` para conmutación por error ante bloqueos de YouTube.

---

## 5. Prevención de Vaciado de Event Loop en Windows (Keepalive del ShardingManager)

### Problema
En Windows, tras instanciar los shards, el proceso gestor (`launcher.js`) dependía únicamente de las referencias de los subprocesos. Si el canal IPC tenía un retraso, Node.js podía considerar vacío el bucle de eventos del proceso padre y cerrarse inesperadamente.

### Modificación
* **Archivo modificado:** [`src/setup/launcher.js`](file:///c:/Users/Usuario/Documents/antigravity/amazing-franklin/MusicBot/src/setup/launcher.js)
  * Se añadió un `setInterval` de latido periódico que garantiza que el bucle de eventos nunca se vacíe.
  * Se añadieron manejadores globales para `uncaughtException` y `unhandledRejection` en el proceso gestor para capturar y reportar cualquier anomalía.
