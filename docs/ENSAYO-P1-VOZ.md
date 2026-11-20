# Ensayo P1 de voz en Discord

Preparar una aplicación y un servidor de prueba aislados de producción. Copiar `.env.test.example` a `.env.test` en la raíz y completar el token del bot de prueba, guild, canal, ID del locutor y diez duraciones medidas en milisegundos. El archivo es local y no debe compartirse.

Desde `rust/`, con el locutor en el canal, ejecutar:

```powershell
cargo test --locked -p player --test driver_manual -- --ignored --nocapture
cargo test --locked -p bot --test recepcion_manual -- --ignored --nocapture
```

Una persona debe confirmar que el WAV se oye durante la primera prueba. En la segunda, esperar la indicación de cada intento y pronunciar diez frases separadas; el ensayo se repite después de reconectar y compara la duración de cada fragmento con su referencia (tolerancia de 100 ms). Registrar por separado tiempos, fragmentos observados y fallos. La duración esperada debe medirse previamente, no inferirse de un resultado del mismo ensayo.

Después arrancar el bot de prueba, ejecutar `/play` en un canal frío y comprobar audio, «ignite pausa», reanudación, estado REST y dashboard. Verificar que otro bot hablando o reproduciendo no se admite como locutor. Anotar versión, hash del ejecutable, fecha y resultados en `paridad-f7.md`. P1 requiere estas comprobaciones adicionales: los dos smoke tests por sí solos no aprueban la puerta.
