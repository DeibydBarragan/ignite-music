//! Orquestación de reproducción: arranque de la pista actual en el driver.
//!
//! Este módulo conecta la cola en memoria con Songbird: dado el primer elemento
//! de la cola, lo reproduce a través de `yt-dlp` sobre una entrada HTTP.

use crate::cola::Cola;
use crate::driver::SongbirdDriver;
use crate::driver_error::DriverError;
use crate::GestorColas;

mod efectos;
mod transiciones;
pub use efectos::{buscar_con_driver, fijar_filtro_con_driver};
pub use transiciones::{
    alternar_pausa_con_driver, cambiar_pista_con_driver, fijar_pausa_con_driver,
    fijar_volumen_con_driver, parar_con_driver,
};

fn parse_guild_id(guild_id: &str) -> Result<u64, DriverError> {
    guild_id
        .parse::<u64>()
        .map_err(|_| DriverError::Input(format!("Invalid guild id: {guild_id}")))
}

/// Devuelve la URL de la primera canción de la cola, si existe y no está detenida.
pub fn primera_cancion_url(cola: &Cola) -> Option<String> {
    if cola.detenida {
        return None;
    }
    cola.canciones.first().map(|elemento| {
        let cancion = &elemento.cancion;
        if cancion.fuente == "spotify" {
            format!(
                "ytsearch1:{}",
                medios::spotify_modelos::create_search_query(&cancion.artista, &cancion.nombre)
            )
        } else {
            cancion.url.clone()
        }
    })
}

/// Reproduce la primera canción de la cola con `yt-dlp`.
///
/// Devuelve `true` si arrancó la reproducción (o la cola estaba en reproducción),
/// `false` si no había nada que reproducir.
pub async fn reproducir_primera_cancion(
    driver: &SongbirdDriver,
    colas: &GestorColas,
    guild_id: &str,
    ytdlp_bin: &str,
    ytdlp_args: Vec<String>,
) -> Result<bool, DriverError> {
    let cola_arc = colas.obtener_cola(guild_id).await;
    let mut cola = cola_arc.lock().await;
    iniciar_pista(driver, &mut cola, ytdlp_bin, ytdlp_args).await
}

/// El llamador mantiene el mutex de la cola durante la instalación del handle.
pub async fn iniciar_pista(
    driver: &SongbirdDriver,
    cola: &mut Cola,
    ytdlp_bin: &str,
    ytdlp_args: Vec<String>,
) -> Result<bool, DriverError> {
    iniciar_pista_interno(driver, cola, ytdlp_bin, ytdlp_args, None, None, false).await
}

/// Variante que comparte el limitador configurado por la aplicación.
pub async fn iniciar_pista_con_settings(
    driver: &SongbirdDriver,
    cola: &mut Cola,
    ytdlp_bin: &str,
    ytdlp_args: Vec<String>,
    settings: &medios::ytdlp_args::YtDlpSettings,
) -> Result<bool, DriverError> {
    iniciar_pista_interno(
        driver,
        cola,
        ytdlp_bin,
        ytdlp_args,
        None,
        Some(settings),
        false,
    )
    .await
}

/// Instala la pista con el filtro vigente y una posición inicial opcional.
pub async fn iniciar_pista_desde(
    driver: &SongbirdDriver,
    cola: &mut Cola,
    ytdlp_bin: &str,
    ytdlp_args: Vec<String>,
    seek_segundos: Option<u64>,
) -> Result<bool, DriverError> {
    iniciar_pista_interno(
        driver,
        cola,
        ytdlp_bin,
        ytdlp_args,
        seek_segundos,
        None,
        false,
    )
    .await
}

/// Reinstala la pista por seek o cambio de filtro. No es un arranque nuevo, así que no registra historial.
pub async fn reiniciar_pista_desde_con_settings(
    driver: &SongbirdDriver,
    cola: &mut Cola,
    ytdlp_bin: &str,
    ytdlp_args: Vec<String>,
    seek_segundos: Option<u64>,
    settings: &medios::ytdlp_args::YtDlpSettings,
) -> Result<bool, DriverError> {
    iniciar_pista_interno(
        driver,
        cola,
        ytdlp_bin,
        ytdlp_args,
        seek_segundos,
        Some(settings),
        true,
    )
    .await
}

/// Variante con seek que conserva el limitador y timeout de la configuración.
pub async fn iniciar_pista_desde_con_settings(
    driver: &SongbirdDriver,
    cola: &mut Cola,
    ytdlp_bin: &str,
    ytdlp_args: Vec<String>,
    seek_segundos: Option<u64>,
    settings: &medios::ytdlp_args::YtDlpSettings,
) -> Result<bool, DriverError> {
    iniciar_pista_interno(
        driver,
        cola,
        ytdlp_bin,
        ytdlp_args,
        seek_segundos,
        Some(settings),
        false,
    )
    .await
}

async fn iniciar_pista_interno(
    driver: &SongbirdDriver,
    cola: &mut Cola,
    ytdlp_bin: &str,
    ytdlp_args: Vec<String>,
    seek_segundos: Option<u64>,
    settings: Option<&medios::ytdlp_args::YtDlpSettings>,
    reinicio: bool,
) -> Result<bool, DriverError> {
    let guild_u64 = parse_guild_id(&cola.guild_id)?;
    let Some(url) = primera_cancion_url(cola) else {
        return Ok(false);
    };

    driver
        .play_ytdl_filtrado(
            guild_u64,
            &url,
            ytdlp_bin,
            ytdlp_args,
            crate::driver_reproduccion::OpcionesReproduccion {
                volumen: cola.volumen,
                pausada: cola.pausada,
                filtro: cola.filtros.first().cloned(),
                seek_segundos,
                limiter: settings.map(|settings| settings.limiter.clone()),
                limiter_timeout_ms: settings.map_or(30_000, |settings| settings.timeout_ms),
                reinicio,
            },
        )
        .await?;
    crate::al_iniciar_cancion(cola);
    Ok(true)
}
