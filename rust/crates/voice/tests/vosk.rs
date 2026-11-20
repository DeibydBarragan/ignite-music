use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use voice::VoskModelManager;

fn root_dir() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .expect("Project root not found")
        .to_path_buf()
}

fn dll_path() -> PathBuf {
    root_dir().join("bin").join("vosk").join("libvosk.dll")
}

fn model_dir() -> PathBuf {
    root_dir()
        .join("data")
        .join("models")
        .join("vosk-model-small-en-us-0.15")
}

fn fixtures_dir() -> PathBuf {
    root_dir().join("test").join("fixtures")
}

fn load_reference_json() -> BTreeMap<String, String> {
    let data_str = include_str!("data/vosk-texto.json");
    serde_json::from_str(data_str).expect("Valid vosk-texto.json")
}

#[test]
fn test_vosk_ignite_and_casual_samples() {
    let dll = dll_path();
    let model = model_dir();
    if !dll.exists() || !model.exists() {
        // Skip test if DLL or model is absent in build environment
        return;
    }

    let manager = VoskModelManager::initialize(&dll, &model).expect("VoskModelManager initialize");
    assert!(manager.is_ready());

    let ref_data = load_reference_json();
    let f_dir = fixtures_dir();

    let mut rec = manager.adquirir().expect("acquire recognizer");

    // 1. ignite-sample.wav
    let ignite_wav = fs::read(f_dir.join("ignite-sample.wav")).expect("read ignite-sample.wav");
    assert!(ignite_wav.len() > 44);
    let ignite_text = rec.procesar_enunciado(&ignite_wav[44..]);
    assert_eq!(ignite_text, ref_data["ignite-sample.wav"]);

    // 2. casual-speech-sample.wav
    let casual_wav =
        fs::read(f_dir.join("casual-speech-sample.wav")).expect("read casual-speech-sample.wav");
    assert!(casual_wav.len() > 44);
    let casual_text = rec.procesar_enunciado(&casual_wav[44..]);
    assert_eq!(casual_text, ref_data["casual-speech-sample.wav"]);

    drop(rec);
    manager.destroy();
    assert!(!manager.is_ready());
}

#[test]
fn test_vosk_paridad_todos_los_fixtures() {
    let dll = dll_path();
    let model = model_dir();
    if !dll.exists() || !model.exists() {
        return;
    }

    let manager = VoskModelManager::initialize(&dll, &model).expect("VoskModelManager initialize");
    assert!(manager.is_ready());

    let ref_data = load_reference_json();
    let f_dir = fixtures_dir();

    let mut rec = manager.adquirir().expect("acquire recognizer");

    for (file_name, expected_text) in &ref_data {
        let wav_path = f_dir.join(file_name);
        let wav_bytes = fs::read(&wav_path)
            .unwrap_or_else(|e| panic!("Failed reading {}: {:?}", wav_path.display(), e));
        assert!(wav_bytes.len() > 44, "WAV too small");
        let pcm = &wav_bytes[44..];

        let actual_text = rec.procesar_enunciado(pcm);
        assert_eq!(
            &actual_text, expected_text,
            "Mismatch for fixture {file_name}"
        );
    }

    drop(rec);
    manager.destroy();
}

#[test]
fn test_vosk_has_word() {
    let dll = dll_path();
    let model = model_dir();
    if !dll.exists() || !model.exists() {
        return;
    }

    let manager = VoskModelManager::initialize(&dll, &model).expect("VoskModelManager initialize");
    assert!(manager.has_word("ignite"));
    assert!(manager.has_word("  IGNITE  "));
    assert!(!manager.has_word("palabraqueclaramentenoexiste12345"));
    manager.destroy();
}
