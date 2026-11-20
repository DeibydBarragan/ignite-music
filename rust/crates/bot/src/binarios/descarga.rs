//! Descarga de archivos por HTTP con verificación de integridad y límites (Spec S-32).

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

pub const MIN_BYTES_DEFECTO: u64 = 1024 * 1024;
pub const MAX_BYTES_DEFECTO: u64 = 512 * 1024 * 1024;
pub const TIMEOUT_DEFECTO: Duration = Duration::from_secs(600);

#[derive(Debug, Clone)]
pub struct OpcionesDescarga {
    pub min_bytes: u64,
    pub max_bytes: u64,
    pub sha256_esperado: Option<String>,
    pub timeout: Duration,
}

impl Default for OpcionesDescarga {
    fn default() -> Self {
        Self {
            min_bytes: MIN_BYTES_DEFECTO,
            max_bytes: MAX_BYTES_DEFECTO,
            sha256_esperado: None,
            timeout: TIMEOUT_DEFECTO,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoDescarga {
    pub bytes: u64,
    pub sha256: String,
}

/// Descarga `url` al archivo `destino` de forma continua calculando el SHA-256.
pub async fn descargar_a_fichero(
    cliente: &reqwest::Client,
    url: &str,
    destino: &Path,
    opciones: OpcionesDescarga,
) -> Result<ResultadoDescarga, String> {
    let padre = destino.parent().unwrap_or_else(|| Path::new("."));
    tokio::fs::create_dir_all(padre)
        .await
        .map_err(|e| format!("No se pudo crear carpeta de destino: {}", e))?;

    let nombre_tmp = format!(
        ".tmp-{}-{}-{}",
        std::process::id(),
        fastrand::u64(..),
        destino
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("descarga")
    );
    let ruta_tmp = padre.join(nombre_tmp);

    let resultado = ejecutar_descarga_con_timeout(cliente, url, &ruta_tmp, &opciones).await;

    match resultado {
        Ok(res) => {
            if let Err(e) = tokio::fs::rename(&ruta_tmp, destino).await {
                let _ = tokio::fs::remove_file(&ruta_tmp).await;
                return Err(format!("Error al mover archivo descargado: {}", e));
            }
            Ok(res)
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&ruta_tmp).await;
            Err(e)
        }
    }
}

async fn ejecutar_descarga_con_timeout(
    cliente: &reqwest::Client,
    url: &str,
    ruta_tmp: &PathBuf,
    opciones: &OpcionesDescarga,
) -> Result<ResultadoDescarga, String> {
    tokio::time::timeout(
        opciones.timeout,
        ejecutar_descarga(cliente, url, ruta_tmp, opciones),
    )
    .await
    .map_err(|_| format!("Tiempo límite superado al descargar de {}", url))?
}

async fn ejecutar_descarga(
    cliente: &reqwest::Client,
    url: &str,
    ruta_tmp: &PathBuf,
    opciones: &OpcionesDescarga,
) -> Result<ResultadoDescarga, String> {
    let respuesta = cliente
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Error en petición HTTP a {}: {}", url, e))?;

    let status = respuesta.status();
    if !status.is_success() {
        return Err(format!("HTTP {} for {}", status.as_u16(), url));
    }

    let mut stream = respuesta.bytes_stream();
    let mut file = File::create(ruta_tmp)
        .await
        .map_err(|e| format!("Error creando archivo temporal: {}", e))?;

    let mut hasher = Sha256::new();
    let mut bytes_totales: u64 = 0;

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| format!("Error recibiendo fragmento: {}", e))?;
        bytes_totales += chunk.len() as u64;

        if bytes_totales > opciones.max_bytes {
            return Err(format!(
                "Download from {} exceeds {} bytes",
                url, opciones.max_bytes
            ));
        }

        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Error escribiendo fragmento: {}", e))?;
    }

    file.flush()
        .await
        .map_err(|e| format!("Error cerrando archivo temporal: {}", e))?;
    drop(file);

    if bytes_totales < opciones.min_bytes {
        return Err(format!(
            "Download from {} is too small ({} bytes)",
            url, bytes_totales
        ));
    }

    let digest = format!("{:x}", hasher.finalize());
    if let Some(ref esperado) = opciones.sha256_esperado {
        if !digest.eq_ignore_ascii_case(esperado) {
            return Err(format!(
                "Checksum mismatch for {} (expected {}, got {})",
                url, esperado, digest
            ));
        }
    }

    Ok(ResultadoDescarga {
        bytes: bytes_totales,
        sha256: digest,
    })
}

/// Obtiene el hash SHA-256 desde un archivo SHA2-256SUMS publicado por la base de descargas.
pub async fn asset_checksum(
    asset: &str,
    base_url: &str,
    cliente: &reqwest::Client,
) -> Option<String> {
    let url = format!("{}/SHA2-256SUMS", base_url.trim_end_matches('/'));
    let res = cliente
        .get(&url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .ok()?;
    if !res.status().is_success() {
        return None;
    }
    let texto = res.text().await.ok()?;
    for linea in texto.lines() {
        let trimmed = linea.trim();
        if trimmed.ends_with(asset) {
            if let Some(hash) = trimmed.split_whitespace().next() {
                return Some(hash.to_lowercase());
            }
        }
    }
    None
}
