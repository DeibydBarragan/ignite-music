# Spec S-30: Supabase (ajustes de voz, historial y datos de usuario)

- Estado: implementado
- Fase: F6
- Módulo: M7
- Origen en Node (solo lectura): `src/core/supabase.js` (`supabase`, `verifyConnection`), `src/voice/settings-store.js` (`VoiceSettingsStore`, `sanitizeWakeWord`), `src/music/history.js` (`recordPlay`, `loadExclusions`, `getExcludedDates`), `src/music/top-songs.js` (`getTopSongs`, `aggregatePlays`, `completeSong`), `src/handlers/favorite.js` (`handleFavorite`), `src/music/playlists.js` (`fetchUserPlaylists`, `upsertUserPlaylist`), `supabase/migrations/` (cuatro archivos).

## Objetivo

Usar la base de Supabase actual con su esquema congelado, y el mismo comportamiento para ajustes de voz, historial de reproducciones, ranking, favoritos y playlists.

Corrige un fallo: hoy, si falla la escritura de los ajustes de voz, la API responde como guardado (ver Decisiones).

## Decisiones

- El esquema de Supabase está congelado (decisión 8 del plan). Esta spec no cambia tablas ni columnas. Los cambios de políticas y de migraciones son cambios aparte, fuera de Rust.
- Fallo al guardar ajustes: 503 `DB_UNAVAILABLE`, sin cambiar la caché ni escribir los bindings (ver Comportamiento 5). Node responde como si hubiera guardado.
- Escritura de `guild_voice_settings` y `guild_voice_bindings` solo para `service_role` y para los propietarios. Hoy, `20261019_voice_commands.sql` da `USING (true)` y `WITH CHECK (true)` a `authenticated`. Cualquier cuenta con sesión puede cambiar los ajustes de cualquier servidor. Es un cambio de base de datos, sin plazo fijado.
- La migración de `user_favorites` entra en el repo antes de F7.
- La zona horaria de las exclusiones se verifica en F7.
- Las variables `SUPABASE_*` están en S-01.

## Archivos

- Crear: `rust/crates/bot/src/datos/mod.rs`, `supabase.rs` (cliente con clave de servicio y verificación), `ajustes_voz.rs`, `historial.rs`, `ranking.rs`, `favoritos.rs` y `listas.rs`.
- Crear: `rust/crates/bot/tests/datos.rs` y `rust/crates/bot/tests/fixtures/supabase/` (respuestas grabadas).
- Crear antes de F7, fuera de Rust: migración de `user_favorites` en `supabase/migrations/` (ver Decisiones).
- No tocar: `src/core/supabase.js`, `src/voice/settings-store.js`, `src/music/history.js`, `src/music/top-songs.js`, `src/handlers/favorite.js`, `src/music/playlists.js` y las migraciones existentes de `supabase/migrations/`.

Cada archivo tiene como máximo 250 líneas.

## Contrato

Variables: `SUPABASE_URL`, `SUPABASE_KEY`, `SUPABASE_ANON_KEY` y `SUPABASE_SERVICE_ROLE_KEY`, definidas en S-01. La clave de servicio tiene prioridad sobre `SUPABASE_KEY`, y `SUPABASE_KEY` usa `SUPABASE_ANON_KEY` si falta, como en Node.

Tablas y funciones, tal como están en las migraciones:

- `song_plays` (20261014): `id` UUID; `guild_id`, `title` y `url` obligatorios; `song_id`, `artist`, `album_art`, `user_id`, `username`, `source` (por defecto `youtube`); `duration` (entero, por defecto 0); `played_at` (por defecto ahora). Sin políticas (20261016): solo service role.
- `top_songs(p_guild_id, p_since, p_limit, p_excluded_dates)` (función, 20261016): agrega por `coalesce(url, song_id, title)`, con 50 filas como máximo. Solo service role.
- `bot_status` (20261016): `id` TEXT clave primaria, `api_url`, `updated_at`. Lectura pública. Escritura solo service role.
- `user_playlists` (20261016): `id` UUID, `user_id`, `name`, `source` (por defecto `generic`), `url`, `cover`, `tracks` JSONB, `synced_at` y `created_at`. Único por (`user_id`, `url`). Acceso del propietario según `auth.identities`.
- `guild_voice_settings` (20261019): `guild_id` clave primaria, `enabled`, `wake_word` (por defecto `ignite`), `language` (por defecto `es`), `tts_feedback_enabled` (falso), `tts_templates` JSONB y `updated_at`.
- `guild_voice_bindings` (20261019): `id` UUID, `guild_id` (clave externa con borrado en cascada), `phrase`, `command_id` y `created_at`. Único por (`guild_id`, `phrase`).
- `user_favorites`: no tiene migración en el repo. Su migración se añade antes de F7 (ver Decisiones). Las columnas que usa el código son `id`, `user_id`, `song_id`, `title`, `artist`, `album_art`, `url`, `duration` y `source`.

## Comportamiento

1. Cliente: sin URL o sin clave, Supabase queda desactivado y el bot sigue. Si falta solo una de las dos, avisa. La verificación consulta `song_plays` (limit 1) con 8 segundos de tiempo límite. Devuelve `ok` o el motivo, y nunca falla.
2. Ajustes, lectura: caché de 60 segundos por servidor. Lee `guild_voice_settings` y `guild_voice_bindings`. Si la lectura falla, devuelve los valores por defecto, sin error. Valores por defecto: `enabled` (de la configuración), `wake_word` `ignite`, `language` `es`, `tts_voice` `es-MX-DaliaNeural`, `tts_pitch` 1.0, `tts_rate` 1.0, `tts_feedback_enabled` (de la configuración), plantillas de S-11 y `bindings` vacío.
3. Voz, pitch y velocidad se guardan dentro de `tts_templates`, con las claves `_voice`, `_pitch` y `_rate`. Las claves que empiezan por `_` no son plantillas.
4. `sanitizeWakeWord`: separa por comas, pasa a minúsculas y quita `bot` y los vacíos. Si no queda nada, usa `ignite`.
5. Ajustes, escritura: fusiona con los valores actuales y hace upsert de `guild_voice_settings`. Si el upsert funciona y el cuerpo trae `bindings` como lista, borra todas las filas del servidor y luego inserta las nuevas, sin transacción. Después actualiza la caché.
   Fallo: si el upsert falla, responde 503 `DB_UNAVAILABLE`, no cambia la caché y no escribe los bindings. Node avisa en el log, responde como si hubiera guardado y deja los valores en caché 60 segundos. Después se pierden.
6. Reinicio: borra `guild_voice_bindings`, después `guild_voice_settings`, limpia la caché y devuelve los valores por defecto.
7. Historial (`recordPlay`): no escribe si el usuario tiene modo dev o si el play viene marcado como `devMode`. Campos: `guild_id`, `song_id`, `title` (`Desconocido` por defecto), `artist` (`Desconocido`), `album_art`, `url`, `duration`, `source` (`youtube`), `user_id`, `username` y `played_at`. Primero guarda en el historial local (JSON, máximo 500 por servidor) y luego inserta en `song_plays`. Un error de inserción solo queda en el log de depuración.
8. Exclusiones: `DATA_DIR/history/exclusions.json`, con `{"dates": ["YYYY-MM-DD"]}`. Se carga al arrancar y se recarga cuando cambia el archivo. Un play cuya fecha UTC (los 10 primeros caracteres de `played_at`) está excluida no cuenta en el ranking.
9. Ranking (`getTopSongs`): `week` son 7 días, `month` 30 días, y cualquier otro valor cubre todo el historial. Límite de 1 a 50. Si la función `top_songs` responde, aunque sea sin filas, esa respuesta se devuelve. Si falla, lee hasta 2000 filas recientes de `song_plays` del servidor y las agrega en el bot. Si esa lectura no da filas, usa el historial local. Cada canción lleva `playCount`, `rank` y `lastPlayed`.
10. Favoritos (`favorite.js`): busca por `song_id` o por URL del usuario en `user_favorites`. Si existe, la borra. Si no, la inserta. Sin cliente, avisa de que no está disponible.
11. Playlists: `fetchUserPlaylists` ordena por `created_at` descendente y lee hasta 100. `upsertUserPlaylist` busca por (`user_id`, `url`): actualiza si existe, o inserta. Sin cliente, responde el error `DB_UNAVAILABLE`.
12. La clave de servicio no aparece en logs ni en respuestas.

## Tests

- Ajustes: caché de 60 segundos, valores por defecto cuando falla la lectura, fusión en la escritura, y `sanitizeWakeWord` con `bot`, `Ignite, bot`, cadena vacía y `bot mierda`.
- Escritura con bindings: reemplazo completo.
- Ajustes con upsert fallido: 503 `DB_UNAVAILABLE`, sin cambios en la caché ni en los bindings.
- Historial: campos del registro, modo dev sin escritura y exclusión por fecha UTC.
- Ranking: respuesta de la función, fallo de la función con lectura acotada, sin filas con historial local, y límites de 1 a 50.
- Favoritos: inserta si no existe y borra si existe.
- Playlists: crea, y después actualiza el mismo par (`user_id`, `url`).
- Zona horaria (F7): una reproducción de las 23:30 UTC cae en el mismo día en las exclusiones y en `top_songs`.
- Paridad: los mismos casos contra Node, con respuestas grabadas en `tests/fixtures/supabase/`. Las pruebas no consultan la base real.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, y `npm test` sin cambios.

## Criterio de aceptación

- Un ajuste guardado desde el dashboard aparece en `guild_voice_settings` y en el siguiente `GET /voice`.
- El ranking de un servidor coincide con el de Node para la misma tabla: mismo orden y mismos conteos.
- Sin `SUPABASE_URL`, el bot arranca, los ajustes usan valores por defecto y el historial queda solo en local.
- Ninguna escritura usa la clave anónima.

## Preguntas abiertas

Ninguna. La zona horaria se verifica en F7 (ver Tests).
