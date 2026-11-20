use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use voice::groq::{transcribir_audio, GroqConfig, GroqError, GROQ_USER_AGENT};
use voice::stt::{transcribir_enunciado, SttOptions};
use voice::vosk::VoskError;
use voice::{VoskBackend, VoskModelManager};

async fn start_mock_server(
    status_code: u16,
    status_text: &'static str,
    body: &'static str,
    delay_ms: u64,
) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 16384];
        let n = socket.read(&mut buf).await.unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]).to_string();
        if delay_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
        }
        let response = format!(
            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
            status_code,
            status_text,
            body.len(),
            body
        );
        let _ = socket.write_all(response.as_bytes()).await;
        request
    });
    (format!("http://{}", addr), handle)
}

struct MockVoskBackend {
    return_text: String,
    call_count: AtomicUsize,
}

impl VoskBackend for MockVoskBackend {
    fn load_model(&self, _path: &Path) -> Result<usize, VoskError> {
        Ok(1)
    }
    fn free_model(&self, _model: usize) {}
    fn has_word(&self, _model: usize, _word: &str) -> bool {
        true
    }
    fn new_recognizer(&self, _model: usize, _sample_rate: f32) -> Result<usize, VoskError> {
        Ok(1)
    }
    fn free_recognizer(&self, _rec: usize) {}
    fn accept_waveform(&self, _rec: usize, _pcm: &[u8]) -> i32 {
        1
    }
    fn final_result(&self, _rec: usize) -> String {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        self.return_text.clone()
    }
    fn reset(&self, _rec: usize) {}
    fn set_words(&self, _rec: usize, _enabled: bool) {}
}

fn fake_wav() -> Vec<u8> {
    let mut wav = vec![0u8; 100];
    wav[..4].copy_from_slice(b"RIFF");
    wav[8..12].copy_from_slice(b"WAVE");
    wav
}

#[tokio::test]
async fn test_groq_sin_clave_o_wav_corto() {
    let wav = fake_wav();
    let mut cfg = GroqConfig {
        api_key: String::new(),
        ..Default::default()
    };
    let err = transcribir_audio(&wav, &cfg).await.unwrap_err();
    assert_eq!(err, GroqError::SinClave);

    cfg.api_key = "gsk_test".into();
    let short_audio = vec![0u8; 40];
    let res = transcribir_audio(&short_audio, &cfg).await.unwrap();
    assert_eq!(res.texto, "");
    assert_eq!(res.segmentos.len(), 0);
}

#[tokio::test]
async fn test_groq_mock_200_ok() {
    let json_body = r#"{"text":"ignite reproduce bad bunny","segments":[{"no_speech_prob":0.01,"avg_logprob":-0.15}]}"#;
    let (url, server) = start_mock_server(200, "OK", json_body, 0).await;

    let cfg = GroqConfig {
        api_url: url,
        api_key: "gsk_mock_12345".into(),
        model: "whisper-large-v3-turbo".into(),
        language: Some("es".into()),
        prompt: Some("Ignite command".into()),
        timeout_ms: 5000,
    };

    let wav = fake_wav();
    let res = transcribir_audio(&wav, &cfg).await.unwrap();
    assert_eq!(res.texto, "ignite reproduce bad bunny");
    assert_eq!(res.segmentos.len(), 1);
    assert_eq!(res.segmentos[0].no_speech_prob, 0.01);

    let req_str = server.await.unwrap();
    let req_lower = req_str.to_lowercase();
    assert!(req_lower.contains("authorization: bearer gsk_mock_12345"));
    assert!(req_lower.contains(&format!("user-agent: {}", GROQ_USER_AGENT.to_lowercase())));
    assert!(req_str.contains("name=\"model\"\r\n\r\nwhisper-large-v3-turbo"));
    assert!(req_str.contains("name=\"response_format\"\r\n\r\nverbose_json"));
    assert!(req_str.contains("filename=\"command.wav\""));
}

#[tokio::test]
async fn test_groq_mock_500_estado() {
    let (url, _) =
        start_mock_server(500, "Internal Server Error", "rate limit or failure", 0).await;
    let cfg = GroqConfig {
        api_url: url,
        api_key: "gsk_test".into(),
        ..Default::default()
    };
    let err = transcribir_audio(&fake_wav(), &cfg).await.unwrap_err();
    assert_eq!(err, GroqError::Estado(500, "rate limit or failure".into()));
}

#[tokio::test]
async fn test_groq_mock_tiempo() {
    let (url, _) = start_mock_server(200, "OK", "{}", 200).await;
    let cfg = GroqConfig {
        api_url: url,
        api_key: "gsk_test".into(),
        timeout_ms: 50,
        ..Default::default()
    };
    let err = transcribir_audio(&fake_wav(), &cfg).await.unwrap_err();
    assert_eq!(err, GroqError::Tiempo(50));
}

#[tokio::test]
async fn test_stt_fallback_vosk() {
    let fake_backend = Arc::new(MockVoskBackend {
        return_text: "ignite pausa".into(),
        call_count: AtomicUsize::new(0),
    });
    let vosk_mgr =
        VoskModelManager::initialize_with_backend(fake_backend.clone(), Path::new("dummy"))
            .unwrap();

    let wav = fake_wav();
    let pcm = vec![0u8; 1600];

    // Case 1: Without Groq API key, direct to Vosk
    let opts_no_key = SttOptions {
        api_key: None,
        ..Default::default()
    };
    let res = transcribir_enunciado(&wav, &pcm, &opts_no_key, Some(&vosk_mgr)).await;
    assert_eq!(res.texto, "ignite pausa");
    assert_eq!(fake_backend.call_count.load(Ordering::SeqCst), 1);

    // Case 2: With Groq key but Groq fails (500), falls back to Vosk
    let (url, _) = start_mock_server(500, "Error", "Groq down", 0).await;
    let opts_with_key = SttOptions {
        api_key: Some("gsk_key".into()),
        api_url: Some(url),
        ..Default::default()
    };
    let res2 = transcribir_enunciado(&wav, &pcm, &opts_with_key, Some(&vosk_mgr)).await;
    assert_eq!(res2.texto, "ignite pausa");
    assert_eq!(fake_backend.call_count.load(Ordering::SeqCst), 2);
}
