//! Aseguramiento de la biblioteca C y modelos acústicos de Vosk (Spec S-32).

use super::descarga::{descargar_a_fichero, OpcionesDescarga};
use super::descompresion::extraer_zip;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const VOSK_ZIP_URL: &str =
    "https://github.com/alphacep/vosk-api/releases/download/v0.3.45/vosk-win64-0.3.45.zip";
pub const VOSK_ZIP_SHA256: &str =
    "f1dcc9cca460630f81ea8f71794f69c80bed6556d2a4e6237b5785e1d2dff34b";

pub const MODEL_EN_URL: &str =
    "https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip";
pub const MODEL_EN_SHA256: &str =
    "30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498";

pub const MODEL_ES_URL: &str = "https://alphacephei.com/vosk/models/vosk-model-small-es-0.42.zip";
pub const MODEL_ES_SHA256: &str =
    "09b239888f633ef2f0b4e09736e3d9936acfd810bc65d53fad45261762c6511f";

pub fn ruta_libvosk(bin_dir: &Path) -> PathBuf {
    bin_dir.join("vosk").join("libvosk.dll")
}

pub fn ruta_modelo_en(data_dir: &Path) -> PathBuf {
    data_dir.join("models").join("vosk-model-small-en-us-0.15")
}

pub fn ruta_modelo_es(data_dir: &Path) -> PathBuf {
    data_dir.join("models").join("vosk-model-small-es")
}

pub async fn asegurar_vosk(
    bin_dir: &Path,
    data_dir: &Path,
    cliente: &reqwest::Client,
    override_urls: Option<(&str, &str, &str)>,
) -> Result<PathBuf, String> {
    let lib_path = ruta_libvosk(bin_dir);
    if !lib_path.is_file() {
        let (dll_url, _, _) = override_urls.unwrap_or((VOSK_ZIP_URL, "", ""));
        instalar_dll_vosk(bin_dir, dll_url, cliente).await?;
    }

    let (_, en_url, es_url) = override_urls.unwrap_or(("", MODEL_EN_URL, MODEL_ES_URL));
    let en_path = ruta_modelo_en(data_dir);
    if !modelo_listo(&en_path) {
        let url = if en_url.is_empty() {
            MODEL_EN_URL
        } else {
            en_url
        };
        instalar_modelo(&en_path, url, MODEL_EN_SHA256, cliente).await?;
    }

    let es_path = ruta_modelo_es(data_dir);
    if !modelo_listo(&es_path) {
        let url = if es_url.is_empty() {
            MODEL_ES_URL
        } else {
            es_url
        };
        instalar_modelo(&es_path, url, MODEL_ES_SHA256, cliente).await?;
    }

    Ok(lib_path)
}

fn modelo_listo(dir: &Path) -> bool {
    dir.join("am").join("final.mdl").is_file() || dir.join(".ready").is_file()
}

pub async fn instalar_dll_vosk(
    bin_dir: &Path,
    url: &str,
    cliente: &reqwest::Client,
) -> Result<(), String> {
    let dir_vosk = bin_dir.join("vosk");
    tokio::fs::create_dir_all(&dir_vosk)
        .await
        .map_err(|e| format!("Error creando carpeta vosk: {}", e))?;

    let pid = std::process::id();
    let temp_zip = dir_vosk.join(format!(".vosk-{}.zip", pid));
    let temp_extract = dir_vosk.join(format!(".extract-vosk-{}", pid));

    let hash_esperado = if url == VOSK_ZIP_URL {
        Some(VOSK_ZIP_SHA256.to_string())
    } else {
        None
    };

    let opciones = OpcionesDescarga {
        min_bytes: 5 * 1024 * 1024,
        max_bytes: 128 * 1024 * 1024,
        sha256_esperado: hash_esperado,
        timeout: Duration::from_secs(600),
    };

    descargar_a_fichero(cliente, url, &temp_zip, opciones).await?;
    let res_ext = extraer_zip(&temp_zip, &temp_extract, 200, 128 * 1024 * 1024);
    let _ = tokio::fs::remove_file(&temp_zip).await;
    res_ext?;

    // Copiar archivos extraídos a bin/vosk
    copiar_archivos_recursivo(&temp_extract, &dir_vosk).await?;
    let _ = tokio::fs::remove_dir_all(&temp_extract).await;

    let dll = dir_vosk.join("libvosk.dll");
    if dll.is_file() {
        tracing::info!("Vosk C library installed successfully");
        Ok(())
    } else {
        Err("libvosk.dll not found after archive extraction".into())
    }
}

pub async fn instalar_modelo(
    destino: &Path,
    url: &str,
    hash_oficial: &str,
    cliente: &reqwest::Client,
) -> Result<(), String> {
    let padre = destino.parent().unwrap_or_else(|| Path::new("."));
    tokio::fs::create_dir_all(padre)
        .await
        .map_err(|e| format!("Error creando carpeta models: {}", e))?;

    let pid = std::process::id();
    let temp_zip = padre.join(format!(".model-{}.zip", pid));
    let temp_extract = padre.join(format!(".extract-model-{}", pid));

    let hash_esperado = if url == MODEL_EN_URL || url == MODEL_ES_URL {
        Some(hash_oficial.to_string())
    } else {
        None
    };

    let opciones = OpcionesDescarga {
        min_bytes: 20 * 1024 * 1024,
        max_bytes: 128 * 1024 * 1024,
        sha256_esperado: hash_esperado,
        timeout: Duration::from_secs(600),
    };

    descargar_a_fichero(cliente, url, &temp_zip, opciones).await?;
    let res_ext = extraer_zip(&temp_zip, &temp_extract, 1000, 256 * 1024 * 1024);
    let _ = tokio::fs::remove_file(&temp_zip).await;
    res_ext?;

    // Encontrar carpeta con am/final.mdl
    let sub = buscar_carpeta_modelo(&temp_extract).await;
    let origen = sub.as_deref().unwrap_or(&temp_extract);

    if !origen.join("am").join("final.mdl").is_file() {
        let _ = tokio::fs::remove_dir_all(&temp_extract).await;
        return Err("Model marker final.mdl not found after archive extraction".into());
    }

    let _ = tokio::fs::create_dir_all(destino).await;
    copiar_archivos_recursivo(origen, destino).await?;
    let _ = tokio::fs::remove_dir_all(&temp_extract).await;
    let _ = tokio::fs::write(destino.join(".ready"), "1").await;

    tracing::info!("Vosk acoustic model installed at {}", destino.display());
    Ok(())
}

fn copiar_archivos_recursivo<'a>(
    desde: &'a Path,
    hacia: &'a Path,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
    Box::pin(async move {
        let mut entries = tokio::fs::read_dir(desde)
            .await
            .map_err(|e| format!("Error leyendo directorio: {}", e))?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            let dest_file = hacia.join(entry.file_name());
            if let Ok(ft) = entry.file_type().await {
                if ft.is_dir() {
                    let _ = tokio::fs::create_dir_all(&dest_file).await;
                    copiar_archivos_recursivo(&path, &dest_file).await?;
                } else if !dest_file.is_file() {
                    let _ = tokio::fs::copy(&path, &dest_file).await;
                }
            }
        }
        Ok(())
    })
}

fn buscar_carpeta_modelo<'a>(
    dir: &'a Path,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<PathBuf>> + Send + 'a>> {
    Box::pin(async move {
        if dir.join("am").join("final.mdl").is_file() {
            return Some(dir.to_path_buf());
        }
        let mut entries = tokio::fs::read_dir(dir).await.ok()?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Ok(ft) = entry.file_type().await {
                if ft.is_dir() {
                    let p = entry.path();
                    if let Some(res) = buscar_carpeta_modelo(&p).await {
                        return Some(res);
                    }
                }
            }
        }
        None
    })
}
