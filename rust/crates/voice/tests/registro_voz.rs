use std::fs;
use std::sync::{Arc, Mutex};

use voice::csv_voz::ExportadorCsvVoz;
use voice::registro_voz::{
    AlmacenRegistroVoz, EntradaRegistroVoz, NuevaEntradaRegistroVoz, PublicadorRegistroVoz,
};

#[derive(Default)]
struct MockPublicador {
    eventos: Mutex<Vec<(String, String, EntradaRegistroVoz)>>,
}

impl PublicadorRegistroVoz for MockPublicador {
    fn publicar(&self, guild_id: &str, evento: &str, entrada: &EntradaRegistroVoz) {
        let mut list = self.eventos.lock().unwrap();
        list.push((guild_id.to_string(), evento.to_string(), entrada.clone()));
    }
}

#[test]
fn test_anillo_descarta_101() {
    let store = AlmacenRegistroVoz::default();
    for i in 0..105 {
        let mut entry = NuevaEntradaRegistroVoz::new("g1");
        entry.vosk_heard = Some(format!("phrase_{i}"));
        store.registrar(entry);
    }

    let logs = store.obtener("g1", Some(200));
    assert_eq!(logs.len(), 100);
    assert_eq!(logs[0].vosk_heard, "phrase_104");
    assert_eq!(logs[99].vosk_heard, "phrase_5");
}

#[test]
fn test_obtener_limite_y_orden() {
    let store = AlmacenRegistroVoz::default();
    for i in 0..10 {
        let mut entry = NuevaEntradaRegistroVoz::new("g1");
        entry.vosk_heard = Some(format!("msg_{i}"));
        store.registrar(entry);
    }

    let top3 = store.obtener("g1", Some(3));
    assert_eq!(top3.len(), 3);
    assert_eq!(top3[0].vosk_heard, "msg_9");
    assert_eq!(top3[1].vosk_heard, "msg_8");
    assert_eq!(top3[2].vosk_heard, "msg_7");

    let all = store.obtener("g1", None);
    assert_eq!(all.len(), 10);
    assert_eq!(all[0].vosk_heard, "msg_9");
    assert_eq!(all[9].vosk_heard, "msg_0");

    let over_limit = store.obtener("g1", Some(100));
    assert_eq!(over_limit.len(), 10);
}

#[test]
fn test_limpiar_vacia_anillo() {
    let store = AlmacenRegistroVoz::default();
    store.registrar(NuevaEntradaRegistroVoz::new("g1"));
    store.registrar(NuevaEntradaRegistroVoz::new("g1"));
    assert_eq!(store.obtener("g1", None).len(), 2);

    store.limpiar("g1");
    assert!(store.obtener("g1", None).is_empty());
}

#[test]
fn test_publicacion_sse_voice_log() {
    let mock = Arc::new(MockPublicador::default());
    let store = AlmacenRegistroVoz::new(100, None, Some(mock.clone()));

    let mut entry = NuevaEntradaRegistroVoz::new("g10");
    entry.vosk_heard = Some("ignite reproduce rap".to_string());
    entry.wake_word_spotted = Some(true);
    let recorded = store.registrar(entry);

    let eventos = mock.eventos.lock().unwrap();
    assert_eq!(eventos.len(), 1);
    let (guild_id, event_name, entry_sent) = &eventos[0];
    assert_eq!(guild_id, "g10");
    assert_eq!(event_name, "voice.log");
    assert_eq!(entry_sent.id, recorded.id);
    assert_eq!(entry_sent.vosk_heard, "ignite reproduce rap");
    assert!(entry_sent.wake_word_spotted);
}

#[test]
fn test_servidor_sin_csv_no_genera_archivos() {
    let dir = tempfile::tempdir().expect("tempdir");
    let exporter = Arc::new(ExportadorCsvVoz::new(dir.path(), 50));
    let store = AlmacenRegistroVoz::with_exporter(exporter);

    // CSV not enabled for g1: 60 records must not write any CSV
    for i in 0..60 {
        let mut entry = NuevaEntradaRegistroVoz::new("g1");
        entry.vosk_heard = Some(format!("phrase_{i}"));
        store.registrar(entry);
    }
    let count_before = fs::read_dir(dir.path()).unwrap().count();
    assert_eq!(count_before, 0);

    // Enable CSV for g1: next 50 records triggers flush
    store.fijar_csv("g1", true);
    assert!(store.csv_activo("g1"));

    for i in 0..50 {
        let mut entry = NuevaEntradaRegistroVoz::new("g1");
        entry.vosk_heard = Some(format!("csv_phrase_{i}"));
        store.registrar(entry);
    }
    let count_after = fs::read_dir(dir.path()).unwrap().count();
    assert_eq!(count_after, 1);
}

#[test]
fn test_directorio_invalido_no_tumba_registro() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file_as_dir = dir.path().join("a_file.txt");
    fs::write(&file_as_dir, "bloqueo").unwrap();

    // Trying to use a normal file as a folder fails mkdir
    let exporter = Arc::new(ExportadorCsvVoz::new(file_as_dir.join("subfolder"), 50));
    let store = AlmacenRegistroVoz::with_exporter(exporter);
    store.fijar_csv("g1", true);

    for i in 0..55 {
        let mut entry = NuevaEntradaRegistroVoz::new("g1");
        entry.vosk_heard = Some(format!("fallback_{i}"));
        let res = store.registrar(entry);
        assert!(!res.id.is_empty());
    }

    let recorded = store.obtener("g1", Some(100));
    assert_eq!(recorded.len(), 55);
}
