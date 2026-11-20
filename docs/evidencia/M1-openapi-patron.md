# M1: patrón de ID de Discord en openapi.yaml

## Causa

En YAML, dentro de comillas simples, la barra invertida no se escapa.
`'^\d{17,20}$'` da la regex `^\d{17,20}$` y acepta IDs de 17 a 20 dígitos.
`'^\\d{17,20}$'` da la regex `^\\d{17,20}$`, que busca una barra invertida literal seguida de `d`. Por eso no acepta IDs reales.

## Antes y después

| Línea | Esquema | Antes | Después |
|---|---|---|---|
| 562 | voice/join, `channelId` | `'^\\d{17,20}$'` | `'^\d{17,20}$'` |
| 563 | voice/join, `userId` | `'^\\d{17,20}$'` | `'^\d{17,20}$'` |
| 683 | `VoiceChannel`, `id` | `'^\\d{17,20}$'` | `'^\d{17,20}$'` |
| 792 | `TrackSnapshot`, `userId` | `'^\\d{17,20}$'` | `'^\d{17,20}$'` |

La búsqueda de patrones con doble barra encontró una cuarta ocurrencia, `TrackSnapshot.userId`. También se corrigió.
Los demás patrones de snowflake del archivo ya usaban `\d` y no cambiaron.
Tras el cambio, `Select-String` sobre `openapi.yaml` devuelve 0 coincidencias de `\\d`.

## Prueba

Se añadió a `test/openapi.test.js` el test "OpenAPI snowflake patterns are single-escaped and accept real Discord IDs".
Falla si algún `pattern:` contiene doble barra invertida.
También falla si un patrón de 17 a 20 dígitos no acepta `123456789012345678`.
Antes del cambio, el test falló con `patron con doble barra invertida: ^\\d{17,20}$`.

## Resultados

| Comando | Resultado |
|---|---|
| `npm run check` | OK: 180 files |
| `npm run lint` | sin errores |
| `npm run typecheck` | sin errores |
| `npm test` | 235 tests, 228 pasan, 0 fallan, 7 omitidas |
| `npx --yes @redocly/cli@1.34.5 lint openapi.yaml` | valid, 0 errores, 41 avisos |

Los 41 avisos de Redocly son los mismos que en T7 (`docs/evidencia/T7-openapi.md`). No se corrigieron, porque están fuera de M1.
No se tocaron `rust/` ni `.github/workflows/rust.yml`. No hubo commit, stash ni reset.
