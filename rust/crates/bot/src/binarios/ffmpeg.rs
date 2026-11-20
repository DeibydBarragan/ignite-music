//! Búsqueda y descarga de ffmpeg (Spec S-32).

use super::descarga::{descargar_a_fichero, OpcionesDescarga};
use super::descompresion::descomprimir_gz;
use super::sonda::{sondear, TIEMPO_SONDA_DEFECTO};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const FFMPEG_GZ_URL: &str =
    "https://github.com/eugeneware/ffmpeg-static/releases/download/b6.1.1/ffmpeg-win32-x64.gz";
pub const FFMPEG_GZ_SHA256: &str =
    "8883a3dffbd0a16cf4ef95206ea05283f78908dbfb118f73c83f4951dcc06d77";
pub const FFMPEG_EXE_SHA256: &str =
    "04e1307997530f9cf2fe35cba2ca7e8875ca91da02f89d6c7243df819c94ad00";

pub fn ruta_local_ffmpeg(bin_dir: &Path) -> PathBuf {
    bin_dir.join("ffmpeg.exe")
}

pub async fn asegurar_ffmpeg(
    bin_dir: &Path,
    ffmpeg_path_cfg: &str,
    download_url: Option<&str>,
    cliente: &reqwest::Client,
) -> Result<PathBuf, String> {
    if !ffmpeg_path_cfg.trim().is_empty() {
        let sonda = sondear(ffmpeg_path_cfg, &["-version"], TIEMPO_SONDA_DEFECTO).await;
        if sonda.ok {
            return Ok(PathBuf::from(ffmpeg_path_cfg));
        }
        tracing::error!(
            "FFMPEG_PATH=\"{}\" does not work: {}",
            ffmpeg_path_cfg,
            sonda.error
        );
        return Err(format!("FFMPEG_PATH=\"{}\" does not work", ffmpeg_path_cfg));
    }

    let local = ruta_local_ffmpeg(bin_dir);
    if local.is_file() {
        let sonda = sondear(
            &local.to_string_lossy(),
            &["-version"],
            TIEMPO_SONDA_DEFECTO,
        )
        .await;
        if sonda.ok {
            return Ok(local);
        }
    }

    tracing::info!("Downloading ffmpeg...");
    match descargar_ffmpeg(bin_dir, download_url, cliente).await {
        Ok(_) => {
            let sonda = sondear(
                &local.to_string_lossy(),
                &["-version"],
                TIEMPO_SONDA_DEFECTO,
            )
            .await;
            if sonda.ok {
                return Ok(local);
            }
        }
        Err(e) => tracing::warn!("Could not download ffmpeg: {}", e),
    }

    let sistema = sondear("ffmpeg", &["-version"], TIEMPO_SONDA_DEFECTO).await;
    if sistema.ok {
        return Ok(PathBuf::from("ffmpeg"));
    }

    tracing::error!("No working ffmpeg found. Install ffmpeg or set FFMPEG_PATH.");
    Err("ffmpeg is missing (needed to play any audio)".into())
}

pub async fn descargar_ffmpeg(
    bin_dir: &Path,
    download_url: Option<&str>,
    cliente: &reqwest::Client,
) -> Result<(), String> {
    let url = download_url.unwrap_or(FFMPEG_GZ_URL);
    let hash_esperado = if download_url.is_none() {
        Some(FFMPEG_GZ_SHA256.to_string())
    } else {
        None
    };

    let pid = std::process::id();
    let temp_gz = bin_dir.join(format!(".ffmpeg-{}.gz", pid));
    let temp_exe = bin_dir.join(format!(".ffmpeg-{}.tmp", pid));
    let local = ruta_local_ffmpeg(bin_dir);

    let opciones = OpcionesDescarga {
        min_bytes: 1024 * 1024,
        max_bytes: 128 * 1024 * 1024,
        sha256_esperado: hash_esperado,
        timeout: Duration::from_secs(600),
    };

    let _ = tokio::fs::create_dir_all(bin_dir).await;
    descargar_a_fichero(cliente, url, &temp_gz, opciones).await?;

    let res_decomp = descomprimir_gz(&temp_gz, &temp_exe);
    let _ = tokio::fs::remove_file(&temp_gz).await;
    res_decomp?;

    let sonda = sondear(
        &temp_exe.to_string_lossy(),
        &["-version"],
        TIEMPO_SONDA_DEFECTO,
    )
    .await;
    if !sonda.ok {
        let _ = tokio::fs::remove_file(&temp_exe).await;
        return Err(format!("Downloaded ffmpeg does not run: {}", sonda.error));
    }

    if let Err(e) = tokio::fs::rename(&temp_exe, &local).await {
        let _ = tokio::fs::remove_file(&temp_exe).await;
        return Err(format!("Error guardando ffmpeg.exe: {}", e));
    }

    Ok(())
}
