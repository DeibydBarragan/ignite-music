use player::Musica;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use voice::despacho::{despachar, AjustesVoz, SolicitudDespacho};

struct TtsMockMusica {
    queue_active: AtomicBool,
    last_action: Mutex<Option<String>>,
}

impl TtsMockMusica {
    fn new(queue: bool) -> Self {
        Self {
            queue_active: AtomicBool::new(queue),
            last_action: Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl Musica for TtsMockMusica {
    async fn play(
        &self,
        _g: &str,
        _c: Option<&str>,
        _u: &str,
        _q: &str,
        _s: bool,
        _p: Option<usize>,
    ) -> Result<(), String> {
        *self.last_action.lock().unwrap() = Some("play".into());
        Ok(())
    }

    async fn apply_control(&self, _g: &str, act: &str, _v: Option<i64>) -> Result<(), String> {
        *self.last_action.lock().unwrap() = Some(act.into());
        Ok(())
    }

    fn has_active_queue(&self, _g: &str) -> bool {
        self.queue_active.load(Ordering::SeqCst)
    }

    fn user_has_voice_channel(&self, _g: &str, _u: &str) -> bool {
        true
    }

    fn user_voice_channel(&self, _g: &str, _u: &str) -> Option<String> {
        Some("789".into())
    }

    fn guild_exists(&self, _g: &str) -> bool {
        true
    }
}

fn make_solicitud(text: &str) -> SolicitudDespacho {
    SolicitudDespacho {
        guild_id: "123".into(),
        user_id: "456".into(),
        text: text.into(),
        channel_id: Some("789".into()),
    }
}

async fn start_mock_tts() -> (String, tokio::task::JoinHandle<serde_json::Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 4096];
        let n = socket.read(&mut buf).await.unwrap_or(0);
        let raw = String::from_utf8_lossy(&buf[..n]);
        let body = raw
            .split("\r\n\r\n")
            .nth(1)
            .and_then(|b| serde_json::from_str(b).ok())
            .unwrap_or_default();
        let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\r\n{\"ok\":true}";
        let _ = socket.write_all(resp.as_bytes()).await;
        body
    });
    (format!("http://{}", addr), handle)
}

#[tokio::test]
async fn test_despacho_hola_que_hace_fuerza_tts() {
    let (url, handle) = start_mock_tts().await;
    let mock = TtsMockMusica::new(false);
    let sol = make_solicitud("ignite hola que hace");
    let cfg = AjustesVoz {
        enabled: true,
        tts_feedback_enabled: false, // Forzado aun estando apagado
        sound_api_url: Some(url),
        ..Default::default()
    };
    let res = despachar(&mock, &sol, Some(&cfg)).await;
    assert!(res.ejecutado);
    assert_eq!(res.comando, Some("hola_que_hace".into()));
    let body = handle.await.unwrap();
    assert_eq!(body["text"], "Hola que hace");
}

#[tokio::test]
async fn test_despacho_custom_templates_y_tts() {
    let (url, handle) = start_mock_tts().await;
    let mock = TtsMockMusica::new(false);
    let sol = make_solicitud("ignite reproduce starboy");
    let mut templates = std::collections::HashMap::new();
    templates.insert("play".to_string(), "Dale play a {query}".to_string());
    let cfg = AjustesVoz {
        enabled: true,
        tts_feedback_enabled: true,
        tts_templates: Some(templates),
        sound_api_url: Some(url),
        tts_voice: Some("es-MX-DaliaNeural".into()),
        tts_rate: Some(1.2),
        tts_pitch: Some(0.9),
        ..Default::default()
    };
    let res = despachar(&mock, &sol, Some(&cfg)).await;
    assert!(res.ejecutado);
    assert_eq!(res.mensaje_tts, Some("Dale play a starboy".into()));
    let body = handle.await.unwrap();
    assert_eq!(body["text"], "Dale play a starboy");
    assert_eq!(body["voice"], "es-MX-DaliaNeural");
    assert_eq!(body["rate"], 1.2);
    assert_eq!(body["pitch"], 0.9);
}
