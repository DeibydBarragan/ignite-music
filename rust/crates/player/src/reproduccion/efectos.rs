use super::*;
use crate::filtros::fijar_filtro_cola;
use crate::ResultadoAccion;

/// Actualiza el filtro y, si hay un handle activo, lo reinstala desde su posición.
/// Revierte filtro y posición si Songbird no acepta la nueva pista.
pub async fn fijar_filtro_con_driver(
    driver: Option<&SongbirdDriver>,
    cola: &mut Cola,
    nombre: &str,
    ytdlp_bin: &str,
    ytdlp_args: Vec<String>,
    settings: &medios::ytdlp_args::YtDlpSettings,
) -> Result<ResultadoAccion, DriverError> {
    let filtros_anteriores = cola.filtros.clone();
    let posicion_anterior = cola.posicion_segundos;
    let resultado = fijar_filtro_cola(cola, nombre);
    let accion = if !resultado.ok {
        ResultadoAccion::no(resultado.clave_mensaje)
    } else if resultado.etiqueta.is_some() {
        ResultadoAccion::si_con_detalle(resultado.clave_mensaje, nombre)
    } else {
        ResultadoAccion::si(resultado.clave_mensaje)
    };
    if !resultado.ok || !resultado.cambio {
        return Ok(accion);
    }
    let Some(driver) = driver else {
        return Ok(accion);
    };
    let Ok(guild_id) = cola.guild_id.parse::<u64>() else {
        cola.filtros = filtros_anteriores;
        return Err(DriverError::Input(format!(
            "Invalid guild id: {}",
            cola.guild_id
        )));
    };
    if let Some(handle) = driver.active_track(guild_id) {
        let posicion = match tokio::time::timeout(
            std::time::Duration::from_secs(1),
            handle.get_info(),
        )
        .await
        {
            Ok(Ok(info)) => info.position.as_secs(),
            _ => posicion_anterior,
        };
        cola.posicion_segundos = posicion;
        match reiniciar_pista_desde_con_settings(
            driver,
            cola,
            ytdlp_bin,
            ytdlp_args,
            Some(posicion),
            settings,
        )
        .await
        {
            Ok(true) => {}
            Ok(false) => {
                cola.filtros = filtros_anteriores;
                cola.posicion_segundos = posicion_anterior;
                return Err(DriverError::NoTrack(guild_id));
            }
            Err(error) => {
                cola.filtros = filtros_anteriores;
                cola.posicion_segundos = posicion_anterior;
                return Err(error);
            }
        }
    }
    Ok(accion)
}

/// Valida seek, actualiza cola y reinstala el handle desde la posición solicitada.
/// Sin driver, conserva el comportamiento puro de la cola.
pub async fn buscar_con_driver(
    driver: Option<&SongbirdDriver>,
    cola: &mut Cola,
    segundos: u64,
    ytdlp_bin: &str,
    ytdlp_args: Vec<String>,
    settings: &medios::ytdlp_args::YtDlpSettings,
) -> Result<ResultadoAccion, DriverError> {
    let anterior = cola.posicion_segundos;
    let resultado = crate::acciones::buscar(cola, segundos);
    if !resultado.ok {
        return Ok(resultado);
    }
    let Some(driver) = driver else {
        return Ok(resultado);
    };
    let guild_id = parse_guild_id(&cola.guild_id)?;
    if driver.active_track(guild_id).is_none() {
        cola.posicion_segundos = anterior;
        return Err(DriverError::NoTrack(guild_id));
    }
    match reiniciar_pista_desde_con_settings(
        driver,
        cola,
        ytdlp_bin,
        ytdlp_args,
        Some(segundos),
        settings,
    )
    .await
    {
        Ok(true) => Ok(resultado),
        Ok(false) => {
            cola.posicion_segundos = anterior;
            Err(DriverError::NoTrack(guild_id))
        }
        Err(error) => {
            cola.posicion_segundos = anterior;
            Err(error)
        }
    }
}
