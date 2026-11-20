# T5: estado gate_unavailable en la ruta de voz de Rust

## Resultado

La ruta de voz de Rust ya cumplía el requisito antes de este cambio. Si el
filtro de palabra de activación (wake gate, Vosk) falta o no está listo, la
admisión devuelve `GateFilteredOut` con `gate_unavailable`. Ese camino no llama
a Groq, no consume el cooldown y deja el registro con `gateReason =
gate_unavailable` y `status = filtered_out`. Node hace lo mismo en
`src/voice/admission.js`: un gate cerrado escribe `filtered_out` y no llama a
Groq.

El cambio añade dos pruebas unitarias que faltaban en `admission.rs`. No se
modificó código de producción.

## Comparación con Node

| Caso | Node (`src/voice/admission.js`) | Rust (`voice/src/admission.rs`) |
|---|---|---|
| Gate no listo | `gate_unavailable`, `filtered_out`, sin Groq | `gate_unavailable`, `filtered_out`, sin Groq |
| Gate cerrado | motivo del gate, `filtered_out`, sin Groq | motivo del gate, `filtered_out`, sin Groq |
| Gating desactivado | sin gate, sigue al cooldown | sin gate, sigue al cooldown |

## Archivos tocados

- `rust/crates/voice/src/admission.rs`: dos pruebas nuevas en `mod tests`
  (`gate_no_disponible_no_admite_y_no_gasta_cooldown`,
  `gating_desactivado_admite_sin_evaluador`). Tamaño del archivo: menos de 250 líneas.

No se tocaron `comandos_voz.rs`, archivos de Node ni otros módulos. No hubo
commit, stash, reset ni checkout.

## Cobertura de los casos pedidos

- (a) Gate no disponible, cero llamadas a Groq, `gate_unavailable` registrado:
  `comandos_voz::pruebas::la_compuerta_bloquea_groq_y_el_fallback_pausa_una_vez`
  (ya existía) y `gate_no_disponible_no_admite_y_no_gasta_cooldown` (nueva).
- (b) Gate disponible y cerrado, cero llamadas a Groq: la misma prueba existente
  con `Gate(false)`, motivo `no_wake_word`.
- (c) Camino normal: la misma prueba con `Gate(true)` comprueba `executed`,
  una llamada a Groq, y `rate_limited` en la segunda petición.
  `gating_desactivado_admite_sin_evaluador` cubre el camino sin gate.

## Comandos y resultados

Todos se ejecutaron en `rust/` salvo npm, que se ejecutó en la raíz.

| Comando | Resultado |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --all-targets -- -D warnings` | exit 0 |
| `cargo test --workspace` | exit 0; 80 suites con `ok`, 328 pruebas pasan, 0 fallan |
| `cargo build --release` | exit 0 |
| `npm test` (raíz) | exit 0; 234 pruebas, 227 pasan, 0 fallan, 7 omitidas |

Las 7 pruebas omitidas de npm ya estaban omitidas antes de este cambio.

## Pendiente

No comprobé que la entrada aparezca en el archivo `voice.log` en disco. Las
pruebas usan un almacén de registro sin ruta de archivo. Tampoco hay prueba en
Discord con DAVE; eso está fuera de este ítem.
