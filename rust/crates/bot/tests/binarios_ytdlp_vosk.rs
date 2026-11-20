//! Pruebas de instalación, límites zip y modelos Vosk (Spec S-32).

use bot::binarios::descompresion::extraer_zip;
use bot::binarios::vosk::{asegurar_vosk, ruta_modelo_en, ruta_modelo_es};
use bot::binarios::ytdlp::{actualizar_ytdlp, marcador_actualizacion, ruta_local_ytdlp};
use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

fn crear_zip_prueba(archivos: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buf);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (nombre, bytes) in archivos {
            zip.start_file(*nombre, opts).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }
    buf.into_inner()
}

#[test]
fn test_zip_mas_entradas_de_las_permitidas_falla() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("muchas_entradas.zip");
    let dest = dir.path().join("extraido");

    let mut entradas: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..10 {
        entradas.push((format!("archivo_{}.txt", i), b"dato".to_vec()));
    }
    let lista_ref: Vec<(&str, &[u8])> = entradas
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect();

    let bytes = crear_zip_prueba(&lista_ref);
    fs::write(&zip_path, bytes).unwrap();

    // Limitar a máximo 5 entradas
    let res = extraer_zip(&zip_path, &dest, 5, 1024 * 1024);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("excede el máximo"));
}

#[test]
fn test_zip_mas_bytes_extraidos_de_los_permitidos_falla() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("pesado.zip");
    let dest = dir.path().join("extraido");

    let bytes = crear_zip_prueba(&[("pesado.bin", &vec![0u8; 1000])]);
    fs::write(&zip_path, bytes).unwrap();

    // Limitar a máximo 500 bytes extraídos
    let res = extraer_zip(&zip_path, &dest, 100, 500);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("excede el máximo"));
}

#[tokio::test]
async fn test_actualizacion_marca_menor_24h_no_consulta() {
    let dir = tempdir().unwrap();
    let bin_dir = dir.path().join("bin");
    let data_dir = dir.path().join("data");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&data_dir).unwrap();

    // Crear un yt-dlp local simulado
    let local = ruta_local_ytdlp(&bin_dir);
    fs::create_dir_all(local.parent().unwrap()).unwrap();
    fs::write(&local, b"ytdlp_dummy").unwrap();

    // Escribir marcador con fecha actual (recién comprobado)
    let marcador = marcador_actualizacion(&data_dir);
    fs::write(&marcador, chrono::Utc::now().to_rfc3339()).unwrap();

    let cliente = reqwest::Client::new();
    // Al tener menos de 24h, debe retornar Ok(None) inmediatamente sin hacer peticiones
    let res = actualizar_ytdlp(&bin_dir, &data_dir, None, &cliente).await;
    assert_eq!(res, Ok(None));
}

#[tokio::test]
async fn test_vosk_modelos_listos_no_se_descargan() {
    let dir = tempdir().unwrap();
    let bin_dir = dir.path().join("bin");
    let data_dir = dir.path().join("data");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&data_dir).unwrap();

    // Crear DLL falsa
    let vosk_dir = bin_dir.join("vosk");
    fs::create_dir_all(&vosk_dir).unwrap();
    fs::write(vosk_dir.join("libvosk.dll"), b"fake_dll").unwrap();

    // Crear modelo en inglés con am/final.mdl
    let en_path = ruta_modelo_en(&data_dir);
    fs::create_dir_all(en_path.join("am")).unwrap();
    File::create(en_path.join("am").join("final.mdl")).unwrap();

    // Crear modelo en español con .ready
    let es_path = ruta_modelo_es(&data_dir);
    fs::create_dir_all(&es_path).unwrap();
    File::create(es_path.join(".ready")).unwrap();

    let cliente = reqwest::Client::new();
    // No debe intentar descargar nada de la red porque ya están listos
    let res = asegurar_vosk(&bin_dir, &data_dir, &cliente, None).await;
    assert!(res.is_ok());
    assert_eq!(res.unwrap(), vosk_dir.join("libvosk.dll"));
}

#[tokio::test]
async fn test_vosk_modelo_ingles_solo_listo_con_final_mdl_o_ready() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().join("data");
    let en_path = ruta_modelo_en(&data_dir);
    fs::create_dir_all(&en_path).unwrap();

    // Sin .ready ni final.mdl: no está listo
    assert!(!en_path.join("am").join("final.mdl").is_file() && !en_path.join(".ready").is_file());

    // Con final.mdl: listo
    fs::create_dir_all(en_path.join("am")).unwrap();
    File::create(en_path.join("am").join("final.mdl")).unwrap();
    assert!(en_path.join("am").join("final.mdl").is_file());
}
