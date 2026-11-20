use std::fs;
use std::path::PathBuf;
use voice::audio::pcm_rms;
use voice::wake_gate_gramatica::build_grammar;
use voice::WakeGate;

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

fn en_model_dir() -> PathBuf {
    root_dir()
        .join("data")
        .join("models")
        .join("vosk-model-small-en-us-0.15")
}

fn es_model_dir() -> PathBuf {
    root_dir()
        .join("data")
        .join("models")
        .join("vosk-model-small-es")
}

fn fixtures_dir() -> PathBuf {
    root_dir().join("test").join("fixtures")
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PuertaCaso {
    file: String,
    palabras: String,
    abierta: bool,
    motivo: String,
    oido: String,
    palabra: Option<String>,
}

#[test]
fn test_wake_gate_not_ready() {
    let gate = WakeGate::new();
    let res = gate.evaluar(&[0u8; 32000], "ignite");
    assert!(!res.abierta);
    assert_eq!(res.motivo, "gate_unavailable");
}

#[test]
fn test_wake_gate_rms_silence_and_speech() {
    assert_eq!(pcm_rms(&[]), 0.0);
    assert_eq!(pcm_rms(&[0i16; 16000]), 0.0);

    let wav = fs::read(fixtures_dir().join("ignite-sample.wav")).expect("read ignite-sample.wav");
    let pcm: Vec<i16> = wav[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .copied()
        .map(i16::from_le_bytes)
        .collect();
    assert!(pcm_rms(&pcm) > 1000.0);
}

#[test]
fn test_wake_gate_grammar_ignite() {
    let is_known = |m: &str, w: &str| {
        m == "en" && matches!(w, "ignite" | "ignore" | "excite" | "invite" | "oye" | "hey")
    };
    let has_model = |m: &str| m == "en";
    let plan_res = build_grammar("ignite", is_known, has_model);
    assert_eq!(plan_res.supported, vec!["ignite"]);
    assert_eq!(plan_res.plans.len(), 1);
    let g = &plan_res.plans[0].grammar;
    assert!(g.contains(&"ignore".to_string()));
    assert!(g.contains(&"[unk]".to_string()));
    assert!(!g.contains(&"bot".to_string()));
}

#[test]
fn test_wake_gate_unsupported_word() {
    let dll = dll_path();
    let en_dir = en_model_dir();
    if !dll.exists() || !en_dir.exists() {
        return;
    }
    let es_dir = es_model_dir();
    let gate = WakeGate::initialize(&dll, &en_dir, Some(&es_dir)).expect("initialize WakeGate");

    let wav = fs::read(fixtures_dir().join("wake-pos-ignite-pausa.wav")).expect("read wav");
    let res = gate.evaluar(&wav[44..], "zxqvyw");
    assert!(!res.abierta);
    assert_eq!(res.motivo, "no_supported_wake_word");
    assert_eq!(res.no_soportadas, Some(vec!["zxqvyw".into()]));
}

#[test]
fn test_wake_gate_paridad_con_node() {
    let dll = dll_path();
    let en_dir = en_model_dir();
    if !dll.exists() || !en_dir.exists() {
        return;
    }
    let es_dir = es_model_dir();
    let mut gate = WakeGate::initialize(&dll, &en_dir, Some(&es_dir)).expect("initialize WakeGate");
    assert!(gate.is_ready());

    let cases_json = include_str!("data/puerta-casos.json");
    let cases: Vec<PuertaCaso> = serde_json::from_str(cases_json).expect("valid puerta-casos.json");

    let f_dir = fixtures_dir();
    for case in &cases {
        let wav_path = f_dir.join(&case.file);
        let wav_bytes = fs::read(&wav_path)
            .unwrap_or_else(|e| panic!("Failed reading {}: {:?}", wav_path.display(), e));
        assert!(wav_bytes.len() > 44);
        let pcm = &wav_bytes[44..];

        let res = gate.evaluar(pcm, &case.palabras);
        assert_eq!(
            res.abierta, case.abierta,
            "abierta mismatch for {} ({})",
            case.file, case.palabras
        );
        assert_eq!(
            res.motivo, case.motivo,
            "motivo mismatch for {} ({})",
            case.file, case.palabras
        );
        assert_eq!(
            res.palabra, case.palabra,
            "palabra mismatch for {} ({})",
            case.file, case.palabras
        );
        assert_eq!(
            res.oido, case.oido,
            "oido mismatch for {} ({})",
            case.file, case.palabras
        );
    }

    gate.destroy();
    assert!(!gate.is_ready());
}
