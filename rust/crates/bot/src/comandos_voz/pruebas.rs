use super::*;
use std::{
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};
use voice::{SttOptions, Transcripcion};

#[derive(Default)]
struct Reproductor(AtomicUsize);

struct GroqFalso(AtomicUsize);

#[async_trait::async_trait]
impl ClienteGroq for GroqFalso {
    async fn transcribir(&self, _: &[u8], _: &SttOptions) -> Option<Transcripcion> {
        self.0.fetch_add(1, Ordering::SeqCst);
        None
    }
}

#[async_trait::async_trait]
impl Musica for Reproductor {
    async fn pausar(&self, _: &str) -> Result<(), player::ErrorMusica> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn user_voice_channel(&self, _: &str, _: &str) -> Option<String> {
        Some("3".into())
    }
    fn guild_exists(&self, _: &str) -> bool {
        true
    }
}

struct Gate(bool);
impl WakeGateEvaluator for Gate {
    fn evaluate(&self, _: &[i16], _: &str) -> voice::GateEvaluation {
        voice::GateEvaluation {
            passed: self.0,
            reason: Some(if self.0 { "matched" } else { "no_wake_word" }.into()),
            ..Default::default()
        }
    }
}

struct Modelo;
impl voice::VoskBackend for Modelo {
    fn load_model(&self, _: &Path) -> Result<usize, voice::VoskError> {
        Ok(1)
    }
    fn free_model(&self, _: usize) {}
    fn has_word(&self, _: usize, _: &str) -> bool {
        true
    }
    fn new_recognizer(&self, _: usize, _: f32) -> Result<usize, voice::VoskError> {
        Ok(1)
    }
    fn free_recognizer(&self, _: usize) {}
    fn accept_waveform(&self, _: usize, _: &[u8]) -> i32 {
        1
    }
    fn final_result(&self, _: usize) -> String {
        "ignite pausa".into()
    }
    fn reset(&self, _: usize) {}
    fn set_words(&self, _: usize, _: bool) {}
}

#[tokio::test]
async fn la_compuerta_bloquea_groq_y_el_fallback_pausa_una_vez() {
    let groq = Arc::new(GroqFalso(AtomicUsize::new(0)));
    let mut config = nucleo::Config::from_map(PathBuf::new(), &Default::default()).voice;
    config.groq_api_key = "loopback-only".into();
    let mut procesador = Procesador {
        config,
        admision: Arc::new(AdmissionController::new(AdmissionConfig::default())),
        gate: None,
        vosk: Some(Arc::new(
            VoskModelManager::initialize_with_backend(Arc::new(Modelo), Path::new("simulado"))
                .unwrap(),
        )),
        registro: Arc::new(AlmacenRegistroVoz::new(100, None, None)),
        inicio: Instant::now(),
        groq: groq.clone(),
    };
    let audio = VoiceFragment {
        guild_id: 1,
        user_id: 2,
        pcm48k: vec![1500; 96_000],
        duration_ms: 1000,
    };
    let musica = Reproductor::default();
    let mut ajustes = AjustesVozGuild::defecto(false, "ignite", false);
    procesador
        .procesar(audio.clone(), ajustes.clone(), &musica, None)
        .await;
    assert!(procesador.registro.obtener("1", None).is_empty());
    ajustes.enabled = true;

    for (gate, motivo) in [
        (None, "gate_unavailable"),
        (
            Some(Arc::new(Gate(false)) as Arc<dyn WakeGateEvaluator>),
            "no_wake_word",
        ),
    ] {
        procesador.gate = gate;
        procesador
            .procesar(audio.clone(), ajustes.clone(), &musica, None)
            .await;
        let entrada = procesador.registro.obtener("1", Some(1)).pop().unwrap();
        assert_eq!(entrada.gate_reason.as_deref(), Some(motivo));
        assert_eq!(entrada.status, "filtered_out");
        assert_eq!(groq.0.load(Ordering::SeqCst), 0);
        assert_eq!(musica.0.load(Ordering::SeqCst), 0);
    }

    procesador.gate = Some(Arc::new(Gate(true)));
    procesador
        .procesar(audio.clone(), ajustes.clone(), &musica, None)
        .await;
    let entrada = procesador.registro.obtener("1", Some(1)).pop().unwrap();
    assert_eq!(entrada.status, "executed");
    assert_eq!(entrada.command_id.as_deref(), Some("pause"));
    assert!(entrada.groq_engaged);
    assert_eq!(entrada.groq_text, None);
    procesador.procesar(audio, ajustes, &musica, None).await;
    assert_eq!(
        procesador.registro.obtener("1", Some(1))[0].status,
        "rate_limited"
    );
    assert_eq!(groq.0.load(Ordering::SeqCst), 1);
    assert_eq!(musica.0.load(Ordering::SeqCst), 1);
}
