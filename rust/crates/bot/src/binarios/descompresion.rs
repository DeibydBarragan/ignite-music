//! Descompresión segura de archivos zip y gz (Spec S-32).

use flate2::read::GzDecoder;
use std::fs::File;
use std::io::{copy, BufReader};
use std::path::Path;

/// Extrae un archivo zip protegiendo contra zip-slip y límites de tamaño.
pub fn extraer_zip(
    zip_path: &Path,
    dest: &Path,
    max_entradas: usize,
    max_bytes: u64,
) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| format!("Error abriendo zip: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Error leyendo cabecera zip: {}", e))?;

    if archive.len() > max_entradas {
        return Err(format!(
            "Archivo zip excede el máximo de {} entradas",
            max_entradas
        ));
    }

    let mut total_extraido: u64 = 0;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Entrada zip {} inválida: {}", i, e))?;

        let encl = match entry.enclosed_name() {
            Some(p) => p.to_owned(),
            None => continue,
        };
        let out_path = dest.join(encl);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| format!("Error creando carpeta en extracción: {}", e))?;
        } else {
            if let Some(p) = out_path.parent() {
                std::fs::create_dir_all(p)
                    .map_err(|e| format!("Error creando carpeta padre: {}", e))?;
            }
            let mut outfile = File::create(&out_path)
                .map_err(|e| format!("Error creando archivo destino: {}", e))?;
            let escritos = copy(&mut entry, &mut outfile)
                .map_err(|e| format!("Error escribiendo archivo extraído: {}", e))?;
            total_extraido += escritos;
            if total_extraido > max_bytes {
                return Err(format!(
                    "Extracción excede el máximo de {} bytes",
                    max_bytes
                ));
            }
        }
    }
    Ok(())
}

/// Descomprime un archivo .gz a un archivo ejecutable destino.
pub fn descomprimir_gz(gz_path: &Path, destino: &Path) -> Result<u64, String> {
    let file = File::open(gz_path).map_err(|e| format!("Error abriendo gz: {}", e))?;
    let mut decoder = GzDecoder::new(BufReader::new(file));
    let mut outfile = File::create(destino).map_err(|e| format!("Error creando destino: {}", e))?;
    copy(&mut decoder, &mut outfile).map_err(|e| format!("Error descomprimiendo gz: {}", e))
}
