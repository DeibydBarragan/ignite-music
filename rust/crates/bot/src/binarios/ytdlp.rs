//! Instalación y actualización de yt-dlp (Spec S-32).

pub use super::descarga::asset_checksum;
use super::descarga::{descargar_a_fichero, OpcionesDescarga};
use super::descompresion::extraer_zip;
use super::sonda::{sondear, TIEMPO_SONDA_DEFECTO};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub const RELEASES_BASE: &str = "https://github.com/yt-dlp/yt-dlp/releases";
pub const ASSET_WIN_X64: &str = "yt-dlp_win.zip";
pub const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_EXTRACTED_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAX_ARCHIVE_ENTRIES: usize = 5000;
pub const UPDATE_INTERVAL_SECS: u64 = 24 * 3600;

pub fn ruta_local_ytdlp(bin_dir: &Path) -> PathBuf {
    bin_dir.join("yt-dlp-dist").join("yt-dlp.exe")
}

pub fn marcador_actualizacion(data_dir: &Path) -> PathBuf {
    data_dir.join("yt-dlp.checked")
}

pub async fn marcar_comprobado(data_dir: &Path) {
    let _ = tokio::fs::create_dir_all(data_dir).await;
    let marcador = marcador_actualizacion(data_dir);
    let ahora = chrono::Utc::now().to_rfc3339();
    let _ = tokio::fs::write(marcador, ahora).await;
}

pub async fn ultima_version_ytdlp(cliente: &reqwest::Client) -> Result<String, String> {
    let url = format!("{}/latest", RELEASES_BASE);
    let res = cliente
        .head(&url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Fallo al consultar última versión de yt-dlp: {}", e))?;

    let final_url = res.url().as_str();
    if let Some(pos) = final_url.find("/tag/") {
        let tag = &final_url[pos + 5..];
        let tag_limpio = tag.split(['/', '?', '#']).next().unwrap_or(tag);
        return Ok(tag_limpio.to_string());
    }
    Err("No se pudo obtener la etiqueta de versión desde la redirección".into())
}

pub async fn asegurar_ytdlp(
    bin_dir: &Path,
    data_dir: &Path,
    ytdlp_path_cfg: &str,
    download_base: Option<&str>,
    cliente: &reqwest::Client,
) -> Result<PathBuf, String> {
    if !ytdlp_path_cfg.trim().is_empty() {
        let sonda = sondear(ytdlp_path_cfg, &["--version"], TIEMPO_SONDA_DEFECTO).await;
        if sonda.ok {
            return Ok(PathBuf::from(ytdlp_path_cfg));
        }
        tracing::error!(
            "YTDLP_PATH=\"{}\" does not work: {}",
            ytdlp_path_cfg,
            sonda.error
        );
        return Err(format!("YTDLP_PATH=\"{}\" does not work", ytdlp_path_cfg));
    }

    let local = ruta_local_ytdlp(bin_dir);
    if local.is_file() {
        let sonda = sondear(
            &local.to_string_lossy(),
            &["--version"],
            TIEMPO_SONDA_DEFECTO,
        )
        .await;
        if sonda.ok {
            return Ok(local);
        }
        tracing::warn!(
            "bin/yt-dlp-dist does not run ({}), downloading it again",
            sonda.error
        );
    }

    tracing::info!("Downloading yt-dlp ({})...", ASSET_WIN_X64);
    match instalar_ytdlp(bin_dir, data_dir, download_base, cliente).await {
        Ok(ver) => {
            tracing::info!("yt-dlp {} installed", ver);
            return Ok(local);
        }
        Err(e) => tracing::error!("Could not install yt-dlp: {}", e),
    }

    let sistema = sondear("yt-dlp", &["--version"], TIEMPO_SONDA_DEFECTO).await;
    if sistema.ok {
        return Ok(PathBuf::from("yt-dlp"));
    }

    tracing::error!(
        "No working yt-dlp found. YouTube will not play. Download it from https://github.com/yt-dlp/yt-dlp/releases and set YTDLP_PATH."
    );
    Err("yt-dlp is missing (needed for YouTube)".into())
}

pub async fn actualizar_ytdlp(
    bin_dir: &Path,
    data_dir: &Path,
    download_base: Option<&str>,
    cliente: &reqwest::Client,
) -> Result<Option<String>, String> {
    let local = ruta_local_ytdlp(bin_dir);
    if !local.is_file() {
        return Ok(None);
    }

    let marcador = marcador_actualizacion(data_dir);
    if let Ok(meta) = tokio::fs::metadata(&marcador).await {
        if let Ok(mtime) = meta.modified() {
            if let Ok(dur) = SystemTime::now().duration_since(mtime) {
                if dur.as_secs() < UPDATE_INTERVAL_SECS {
                    return Ok(None);
                }
            }
        }
    }

    let sonda = sondear(
        &local.to_string_lossy(),
        &["--version"],
        TIEMPO_SONDA_DEFECTO,
    )
    .await;
    let ultima = match ultima_version_ytdlp(cliente).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                "yt-dlp update check failed (will retry on next start): {}",
                e
            );
            return Ok(None);
        }
    };

    if sonda.ok && sonda.salida == ultima {
        marcar_comprobado(data_dir).await;
        return Ok(None);
    }

    tracing::info!("Updating yt-dlp {} -> {}...", sonda.salida, ultima);
    match instalar_ytdlp(bin_dir, data_dir, download_base, cliente).await {
        Ok(v) => {
            tracing::info!("yt-dlp {} installed", v);
            Ok(Some(v))
        }
        Err(e) => {
            tracing::warn!("Failed to update yt-dlp: {}", e);
            Ok(None)
        }
    }
}

pub async fn instalar_ytdlp(
    bin_dir: &Path,
    data_dir: &Path,
    download_base: Option<&str>,
    cliente: &reqwest::Client,
) -> Result<String, String> {
    let base = download_base.unwrap_or("https://github.com/yt-dlp/yt-dlp/releases/latest/download");
    let asset = ASSET_WIN_X64;
    let url = format!("{}/{}", base.trim_end_matches('/'), asset);

    let pid = std::process::id();
    let archivo_zip = bin_dir.join(format!(".yt-dlp-{}.zip", pid));
    let dir_staging = bin_dir.join(format!(".yt-dlp-{}", pid));
    let dir_install = bin_dir.join("yt-dlp-dist");

    let checksum = asset_checksum(asset, base, cliente).await;
    if checksum.is_none() && download_base.is_some() {
        tracing::warn!(
            "No SHA2-256SUMS found for custom base {}, skipping checksum",
            base
        );
    }

    let opciones = OpcionesDescarga {
        min_bytes: 1024 * 1024,
        max_bytes: MAX_ARCHIVE_BYTES,
        sha256_esperado: checksum,
        timeout: Duration::from_secs(600),
    };

    let _ = tokio::fs::create_dir_all(bin_dir).await;
    let _ = tokio::fs::remove_dir_all(&dir_staging).await;

    descargar_a_fichero(cliente, &url, &archivo_zip, opciones).await?;
    let res_extract = extraer_zip(
        &archivo_zip,
        &dir_staging,
        MAX_ARCHIVE_ENTRIES,
        MAX_EXTRACTED_BYTES,
    );
    let _ = tokio::fs::remove_file(&archivo_zip).await;
    res_extract?;

    let ejecutable = dir_staging.join("yt-dlp.exe");
    if !ejecutable.is_file() {
        let _ = tokio::fs::remove_dir_all(&dir_staging).await;
        return Err("The yt-dlp archive does not contain yt-dlp.exe".into());
    }

    let sonda = sondear(
        &ejecutable.to_string_lossy(),
        &["--version"],
        TIEMPO_SONDA_DEFECTO,
    )
    .await;
    if !sonda.ok {
        let _ = tokio::fs::remove_dir_all(&dir_staging).await;
        return Err(format!(
            "the downloaded yt-dlp does not run: {}",
            sonda.error
        ));
    }

    let dir_viejo = bin_dir.join(format!(".yt-dlp-dist.old-{}", pid));
    if dir_install.exists() {
        let _ = tokio::fs::rename(&dir_install, &dir_viejo).await;
    }
    if let Err(e) = tokio::fs::rename(&dir_staging, &dir_install).await {
        let _ = tokio::fs::remove_dir_all(&dir_staging).await;
        return Err(format!("Error renombrando carpeta de instalación: {}", e));
    }
    let _ = tokio::fs::remove_dir_all(&dir_viejo).await;

    for viejo in ["yt-dlp.exe", "yt-dlp"] {
        let ruta = bin_dir.join(viejo);
        if ruta.is_file() {
            let _ = tokio::fs::remove_file(ruta).await;
        }
    }

    marcar_comprobado(data_dir).await;
    Ok(sonda.salida)
}
