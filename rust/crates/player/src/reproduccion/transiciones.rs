use super::*;
use crate::ResultadoAccion;

/// Cambia la pausa de cola y Songbird como una sola operación lógica.
/// Si el driver falla, restaura el estado de la cola.
pub async fn fijar_pausa_con_driver(
    driver: Option<&SongbirdDriver>,
    cola: &mut Cola,
    pausada: bool,
) -> Result<ResultadoAccion, DriverError> {
    let estado_anterior = cola.pausada;
    let resultado = if pausada {
        crate::acciones::pausar(cola)
    } else {
        crate::acciones::reanudar(cola)
    };
    if !resultado.ok {
        return Ok(resultado);
    }
    let Some(driver) = driver else {
        return Ok(resultado);
    };
    let guild_id = match parse_guild_id(&cola.guild_id) {
        Ok(guild_id) => guild_id,
        Err(error) => {
            cola.pausada = estado_anterior;
            return Err(error);
        }
    };
    let efecto = if pausada {
        driver.pause(guild_id).await
    } else {
        driver.resume(guild_id).await
    };
    if let Err(error) = efecto {
        cola.pausada = estado_anterior;
        return Err(error);
    }
    Ok(resultado)
}

/// Alterna pausa a través de la misma operación coordinada con Songbird.
pub async fn alternar_pausa_con_driver(
    driver: Option<&SongbirdDriver>,
    cola: &mut Cola,
) -> Result<ResultadoAccion, DriverError> {
    fijar_pausa_con_driver(driver, cola, !cola.pausada).await
}

/// Aplica skip/previous y detiene el handle actual solo si la cola acepta el cambio.
/// Si Songbird rechaza la transición, restaura la cola completa.
pub async fn cambiar_pista_con_driver(
    driver: Option<&SongbirdDriver>,
    cola: &mut Cola,
    ir_anterior: bool,
) -> Result<ResultadoAccion, DriverError> {
    let estado_anterior = cola.clone();
    let resultado = if ir_anterior {
        crate::acciones::anterior(cola)
    } else {
        crate::acciones::saltar(cola)
    };
    if !resultado.ok {
        return Ok(resultado);
    }
    let Some(driver) = driver else {
        return Ok(resultado);
    };
    let guild_id = match parse_guild_id(&cola.guild_id) {
        Ok(guild_id) => guild_id,
        Err(error) => {
            *cola = estado_anterior;
            return Err(error);
        }
    };
    if let Err(error) = driver.detener_y_avanzar(guild_id).await {
        *cola = estado_anterior;
        return Err(error);
    }
    Ok(resultado)
}

/// Vacía la cola y detiene/sale de Songbird en una sola operación compartida.
pub async fn parar_con_driver(
    driver: Option<&SongbirdDriver>,
    cola: &mut Cola,
) -> Result<ResultadoAccion, DriverError> {
    let estado_anterior = cola.clone();
    let resultado = crate::acciones::parar(cola);
    cola.canal_voz = None;
    let Some(driver) = driver else {
        return Ok(resultado);
    };
    let guild_id = match parse_guild_id(&cola.guild_id) {
        Ok(guild_id) => guild_id,
        Err(error) => {
            *cola = estado_anterior;
            return Err(error);
        }
    };
    if let Err(error) = driver.stop(guild_id).await {
        *cola = estado_anterior;
        return Err(error);
    }
    if let Err(error) = driver.leave(guild_id).await {
        tracing::warn!(guild_id, %error, "Se detuvo la música pero no se pudo salir del canal");
    }
    Ok(resultado)
}

/// Actualiza el volumen en cola y pista; ante error restaura el valor previo.
pub async fn fijar_volumen_con_driver(
    driver: Option<&SongbirdDriver>,
    cola: &mut Cola,
    volumen: i64,
    max_volumen: u8,
) -> Result<ResultadoAccion, DriverError> {
    let anterior = cola.volumen;
    let resultado = crate::acciones::fijar_volumen(cola, volumen, max_volumen);
    if !resultado.ok {
        return Ok(resultado);
    }
    let Some(driver) = driver else {
        return Ok(resultado);
    };
    let guild_id = match parse_guild_id(&cola.guild_id) {
        Ok(guild_id) => guild_id,
        Err(error) => {
            cola.volumen = anterior;
            return Err(error);
        }
    };
    if let Err(error) = driver.set_volume(guild_id, (volumen as f32) / 100.0).await {
        cola.volumen = anterior;
        return Err(error);
    }
    Ok(resultado)
}
