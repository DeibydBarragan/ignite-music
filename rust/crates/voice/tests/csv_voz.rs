use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use voice::csv_voz::{
    build_csv, format_cell, to_csv_cell, ExportadorCsvVoz, CSV_BATCH_SIZE, CSV_COLUMNS,
};
use voice::registro_voz::EntradaRegistroVoz;

#[derive(serde::Deserialize)]
struct CsvCasos {
    #[serde(rename = "cellCases")]
    cell_cases: Vec<CellCase>,
    entries: Vec<EntradaRegistroVoz>,
    #[serde(rename = "expectedCsv")]
    expected_csv: String,
}

#[derive(serde::Deserialize)]
struct CellCase {
    input: Option<serde_json::Value>,
    expected: String,
}

#[test]
fn test_to_csv_cell_escapes() {
    assert_eq!(to_csv_cell("hola"), "hola");
    assert_eq!(to_csv_cell("a,b"), "\"a,b\"");
    assert_eq!(to_csv_cell("di \"hola\""), "\"di \"\"hola\"\"\"");
    assert_eq!(to_csv_cell("a\nb"), "\"a\nb\"");
    assert_eq!(to_csv_cell("a\r\nb"), "\"a\r\nb\"");
    assert_eq!(format_cell(None, false), "");
    assert_eq!(to_csv_cell("0"), "0");
}

#[test]
fn test_node_cell_cases_parity() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("csv-casos.json");
    let content = fs::read_to_string(&path).expect("leer csv-casos.json");
    let casos: CsvCasos = serde_json::from_str(&content).expect("parsear csv-casos.json");

    for c in casos.cell_cases {
        let actual = match c.input {
            None => format_cell(None, false),
            Some(serde_json::Value::Null) => format_cell(None, false),
            Some(serde_json::Value::String(s)) => to_csv_cell(&s),
            Some(serde_json::Value::Number(n)) => to_csv_cell(&n.to_string()),
            Some(serde_json::Value::Bool(b)) => to_csv_cell(&b.to_string()),
            Some(other) => to_csv_cell(&other.to_string()),
        };
        assert_eq!(actual, c.expected);
    }
}

#[test]
fn test_paridad_byte_a_byte_con_node() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("csv-casos.json");
    let content = fs::read_to_string(&path).expect("leer csv-casos.json");
    let casos: CsvCasos = serde_json::from_str(&content).expect("parsear csv-casos.json");

    let actual_csv = build_csv(&casos.entries);
    assert_eq!(actual_csv, casos.expected_csv);
}

#[test]
fn test_formula_injection_guard() {
    assert_eq!(format_cell(Some("=SUM(1,2)"), true), "\"'=SUM(1,2)\"");
    assert_eq!(format_cell(Some("=cmd"), true), "'=cmd");
    assert_eq!(format_cell(Some("+cmd"), true), "'+cmd");
    assert_eq!(format_cell(Some("-cmd"), true), "'-cmd");
    assert_eq!(format_cell(Some("@cmd"), true), "'@cmd");
    assert_eq!(format_cell(Some("\tcmd"), true), "'\tcmd");
    assert_eq!(format_cell(Some("\rcmd"), true), "\"'\rcmd\"");

    // Non-free-text column is not prefixed
    assert_eq!(format_cell(Some("=status"), false), "=status");
    assert_eq!(format_cell(Some("-42"), false), "-42");
    assert_eq!(format_cell(Some("+1"), false), "+1");
}

fn make_dummy_entry(guild_id: &str, id: &str) -> EntradaRegistroVoz {
    EntradaRegistroVoz {
        id: id.to_string(),
        timestamp: "2026-11-08T22:00:00.000Z".to_string(),
        guild_id: guild_id.to_string(),
        user_id: Some("u1".to_string()),
        audio_bytes: 32000,
        vosk_heard: "ignite pausa".to_string(),
        wake_word_spotted: true,
        matched_wake_word: Some("ignite".to_string()),
        gate_reason: Some("ok".to_string()),
        groq_engaged: false,
        groq_text: None,
        groq_elapsed_ms: None,
        command_id: Some("pause".to_string()),
        query: None,
        tts_message: Some("Pausa".to_string()),
        status: "executed".to_string(),
    }
}

#[test]
fn test_exporter_batches_every_50() {
    let dir = tempfile::tempdir().expect("tempdir");
    let exporter = ExportadorCsvVoz::new(dir.path(), CSV_BATCH_SIZE);

    for i in 0..49 {
        let entry = make_dummy_entry("g1", &format!("e{i}"));
        assert!(exporter.add(&entry).is_none());
    }
    let files_0 = fs::read_dir(dir.path()).unwrap().count();
    assert_eq!(files_0, 0);

    let entry_49 = make_dummy_entry("g1", "e49");
    let written = exporter.add(&entry_49);
    assert!(written.is_some());
    let files_1: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("csv"))
        .collect();
    assert_eq!(files_1.len(), 1);

    let content = fs::read_to_string(files_1[0].path()).unwrap();
    let lines: Vec<&str> = content.trim().split('\n').collect();
    assert_eq!(lines.len(), 51); // 1 header + 50 rows
    assert_eq!(lines[0], CSV_COLUMNS.join(","));

    for i in 50..100 {
        let entry = make_dummy_entry("g1", &format!("e{i}"));
        exporter.add(&entry);
    }
    let files_2 = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("csv"))
        .count();
    assert_eq!(files_2, 2);

    for i in 0..49 {
        let entry = make_dummy_entry("g2", &format!("x{i}"));
        exporter.add(&entry);
    }
    let files_after_g2 = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("csv"))
        .count();
    assert_eq!(files_after_g2, 2);
}

#[test]
fn test_flush_all_writes_all_partials() {
    let dir = tempfile::tempdir().expect("tempdir");
    let exporter = ExportadorCsvVoz::new(dir.path(), 50);

    for i in 0..10 {
        exporter.add(&make_dummy_entry("g1", &format!("a{i}")));
    }
    for i in 0..3 {
        exporter.add(&make_dummy_entry("g2", &format!("b{i}")));
    }

    let files_before = fs::read_dir(dir.path()).unwrap().count();
    assert_eq!(files_before, 0);

    let flushed = exporter.flush_all();
    assert_eq!(flushed.len(), 2);
    let files_after = fs::read_dir(dir.path()).unwrap().count();
    assert_eq!(files_after, 2);

    let second_flush = exporter.flush_all();
    assert!(second_flush.is_empty());
}

#[test]
fn test_retencion_borra_antiguos() {
    let dir = tempfile::tempdir().expect("tempdir");
    let exporter = ExportadorCsvVoz::new(dir.path(), 50);

    let e = make_dummy_entry("g1", "e1");
    let path = exporter.flush("g1", &[e]).expect("flush file");
    assert!(path.exists());

    let deleted_30_days = exporter.borrar_antiguos(30);
    assert_eq!(deleted_30_days, 0);
    assert!(path.exists());

    std::thread::sleep(Duration::from_millis(20));
    let deleted = exporter.borrar_mas_antiguos_que(Duration::from_millis(10));
    assert_eq!(deleted, 1);
    assert!(!path.exists());
}

#[tokio::test]
async fn test_tarea_retencion_programada() {
    let dir = tempfile::tempdir().expect("tempdir");
    let exporter = std::sync::Arc::new(ExportadorCsvVoz::new(dir.path(), 50));
    let e = make_dummy_entry("g1", "e1");
    let path = exporter.flush("g1", &[e]).expect("flush file");
    assert!(path.exists());

    let handle = exporter.iniciar_tarea_retencion(0, Duration::from_millis(10));
    tokio::time::sleep(Duration::from_millis(30)).await;
    handle.abort();
    assert!(!path.exists());
}
