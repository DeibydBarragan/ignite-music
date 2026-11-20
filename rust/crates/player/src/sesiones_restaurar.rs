use crate::cola::{Cola, GestorColas, ModoRepeticion};
use crate::sesiones::AlmacenSesiones;

#[async_trait::async_trait]
pub trait ProveedorRestauracion: Send + Sync {
    fn tiene_servidor(&self, guild_id: &str) -> bool;
    async fn canal_voz_valido(&self, guild_id: &str, canal_id: &str) -> bool;
    async fn canal_texto_valido(&self, guild_id: &str, canal_id: &str) -> bool;
    async fn contar_oyentes(&self, guild_id: &str, canal_id: &str) -> usize;
    /// Une el bot al canal de voz y arranca la pista actual de `cola`, igual que /play.
    async fn arrancar_reproduccion(
        &self,
        cola: &mut Cola,
        canal_voz_id: &str,
        seek_segundos: Option<u64>,
    ) -> Result<(), String>;
    /// Espera a que la caché del proveedor tenga los servidores guardados. Por defecto no espera.
    async fn esperar_cache(&self, _guild_ids: &[String]) {}
    /// Sale del canal de voz si el arranque falló tras unirse. Por defecto no hace nada.
    async fn salir_de_voz(&self, _guild_id: &str) {}
}

pub async fn guardar_todas_las_sesiones(
    gestor_colas: &GestorColas,
    almacen: &AlmacenSesiones,
) -> usize {
    let colas = gestor_colas.colas_activas().await;
    let mut guardadas = 0;
    for cola_lock in colas {
        let cola = cola_lock.lock().await;
        if !cola.detenida && !cola.canciones.is_empty() {
            if let Ok(true) = almacen.guardar(&cola).await {
                guardadas += 1;
            }
        }
    }
    guardadas
}

pub async fn restaurar_sesiones<P: ProveedorRestauracion>(
    gestor_colas: &GestorColas,
    almacen: &AlmacenSesiones,
    proveedor: &P,
) -> usize {
    // Sin esto, guild_ids y cargar leen una caché vacía y no restauran nada.
    let _ = almacen.initialize().await;
    let guild_ids = almacen.guild_ids();
    proveedor.esperar_cache(&guild_ids).await;

    let mut restauradas = 0;
    for guild_id in guild_ids {
        if !proveedor.tiene_servidor(&guild_id) {
            continue;
        }

        let estado = restaurar_servidor(gestor_colas, almacen, proveedor, &guild_id).await;
        if estado == Estado::Restaurada {
            restauradas += 1;
        }
        // Sin cola activa, el bot no debe quedarse unido al canal de voz.
        if estado == Estado::ArranqueFallido {
            proveedor.salir_de_voz(&guild_id).await;
        }

        // Divergencia deliberada de Node (src/music/sessions-restore.js, finally):
        // si el arranque falla, la sesión se conserva para reintentar en el siguiente arranque.
        let cola_lock = gestor_colas.obtener_cola(&guild_id).await;
        let cola = cola_lock.lock().await;
        if estado != Estado::ArranqueFallido && !cola.esta_activa() {
            let _ = almacen.eliminar(&guild_id).await;
        }
    }

    if restauradas > 0 {
        tracing::info!("Restored {} music session(s)", restauradas);
    }
    restauradas
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Estado {
    Restaurada,
    Descartada,
    ArranqueFallido,
}

async fn restaurar_servidor<P: ProveedorRestauracion>(
    gestor_colas: &GestorColas,
    almacen: &AlmacenSesiones,
    proveedor: &P,
    guild_id: &str,
) -> Estado {
    let data = match almacen.cargar(guild_id) {
        Some(d) => d,
        None => return Estado::Descartada,
    };
    let voice_id = match &data.voice_channel_id {
        Some(id) if !id.is_empty() => id.clone(),
        _ => return Estado::Descartada,
    };
    if !proveedor.canal_voz_valido(guild_id, &voice_id).await {
        return Estado::Descartada;
    }
    if proveedor.contar_oyentes(guild_id, &voice_id).await == 0 {
        return Estado::Descartada;
    }
    let canciones = almacen.reconstruir_canciones(&data);
    if canciones.is_empty() {
        return Estado::Descartada;
    }
    let text_id = match &data.text_channel_id {
        Some(id) if proveedor.canal_texto_valido(guild_id, id).await => Some(id.clone()),
        _ => None,
    };

    let cola_lock = gestor_colas.obtener_cola(guild_id).await;
    let mut cola = cola_lock.lock().await;
    if cola.esta_activa() {
        return Estado::Descartada;
    }

    cola.canal_voz = Some(voice_id.clone());
    cola.canal_texto = text_id;
    cola.canciones = canciones;
    cola.volumen = data.volume;
    cola.modo_repeticion = ModoRepeticion::desde_u8(data.repeat_mode);
    cola.autoplay = data.autoplay;
    cola.filtros = data.filters.clone();
    cola.pausada = data.paused;
    cola.detenida = false;

    let seek = if let Some(primera) = cola.canciones.first() {
        let dur = primera.cancion.duracion;
        if dur > 0 && data.position > 0 && data.position < dur.saturating_sub(2) {
            cola.posicion_segundos = data.position;
            Some(data.position)
        } else {
            None
        }
    } else {
        None
    };

    match proveedor
        .arrancar_reproduccion(&mut cola, &voice_id, seek)
        .await
    {
        Ok(()) => Estado::Restaurada,
        Err(err) => {
            tracing::warn!(
                "Could not restore session for guild {}: {}. Kept for retry on next start",
                guild_id,
                err
            );
            cola.detenida = true;
            cola.canciones.clear();
            Estado::ArranqueFallido
        }
    }
}
