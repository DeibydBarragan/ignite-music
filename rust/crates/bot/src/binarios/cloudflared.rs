//! Búsqueda y descarga de cloudflared (Spec S-32).

use super::descarga::{descargar_a_fichero, OpcionesDescarga};
use super::sonda::{sondear, TIEMPO_SONDA_DEFECTO};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const CLOUDFLARED_URL: &str =
    "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-windows-amd64.exe";
pub const CLOUDFLARED_SHA256: &str =
    "2493dbc168923bb3e59e5e7716f71deced92992eea9acd6e9d74f84ac7751343";

pub fn ruta_local_cloudflared(bin_dir: &Path) -> PathBuf {
    bin_dir.join("cloudflared.exe")
}

pub async fn asegurar_cloudflared(
    bin_dir: &Path,
    root: &Path,
    auto_tunnel: bool,
    download_url: Option<&str>,
    cliente: &reqwest::Client,
) -> Result<Option<PathBuf>, String> {
    if !auto_tunnel {
        return Ok(None);
    }

    let local = ruta_local_cloudflared(bin_dir);
    if local.is_file() {
        let sonda = sondear(
            &local.to_string_lossy(),
            &["--version"],
            TIEMPO_SONDA_DEFECTO,
        )
        .await;
        if sonda.ok {
            return Ok(Some(local));
        }
        tracing::warn!(
            "Existing cloudflared at {} is invalid ({}), replacing...",
            local.display(),
            sonda.error
        );
        let _ = tokio::fs::remove_file(&local).await;
    }

    let otros_candidatos = [
        root.join("cloudflared.exe"),
        PathBuf::from(r"C:\Program Files (x86)\cloudflared\cloudflared.exe"),
        PathBuf::from(r"C:\Program Files\cloudflared\cloudflared.exe"),
    ];

    for candidato in otros_candidatos {
        if candidato.is_file() {
            let sonda = sondear(
                &candidato.to_string_lossy(),
                &["--version"],
                TIEMPO_SONDA_DEFECTO,
            )
            .await;
            if sonda.ok {
                return Ok(Some(candidato));
            }
        }
    }

    let sistema = sondear("cloudflared", &["--version"], TIEMPO_SONDA_DEFECTO).await;
    if sistema.ok {
        return Ok(Some(PathBuf::from("cloudflared")));
    }

    tracing::info!("Downloading official cloudflared for Windows x64...");
    match descargar_cloudflared(bin_dir, download_url, cliente).await {
        Ok(path) => Ok(Some(path)),
        Err(e) => {
            tracing::warn!(
                "Failed to auto-download cloudflared: {}. Tunnel will not start.",
                e
            );
            Ok(None)
        }
    }
}

pub async fn descargar_cloudflared(
    bin_dir: &Path,
    download_url: Option<&str>,
    cliente: &reqwest::Client,
) -> Result<PathBuf, String> {
    let url = download_url.unwrap_or(CLOUDFLARED_URL);
    let hash_esperado = if download_url.is_none() {
        Some(CLOUDFLARED_SHA256.to_string())
    } else {
        None
    };

    let pid = std::process::id();
    let temp_exe = bin_dir.join(format!(".cloudflared-{}.tmp", pid));
    let local = ruta_local_cloudflared(bin_dir);

    let opciones = OpcionesDescarga {
        min_bytes: 10 * 1024 * 1024,
        max_bytes: 128 * 1024 * 1024,
        sha256_esperado: hash_esperado,
        timeout: Duration::from_secs(600),
    };

    let _ = tokio::fs::create_dir_all(bin_dir).await;
    descargar_a_fichero(cliente, url, &temp_exe, opciones).await?;

    let sonda = sondear(
        &temp_exe.to_string_lossy(),
        &["--version"],
        TIEMPO_SONDA_DEFECTO,
    )
    .await;
    if !sonda.ok {
        let _ = tokio::fs::remove_file(&temp_exe).await;
        return Err(format!(
            "Downloaded cloudflared failed verification: {}",
            sonda.error
        ));
    }

    if let Err(e) = tokio::fs::rename(&temp_exe, &local).await {
        let _ = tokio::fs::remove_file(&temp_exe).await;
        return Err(format!("Error guardando cloudflared.exe: {}", e));
    }

    tracing::info!("cloudflared installed successfully ({})", sonda.salida);
    Ok(local)
}
