use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use voice::tts::{
    interpolate_template, resolve_feedback_message, send_tts_feedback, AvisoTts, OpcionesTts,
};

#[derive(Debug, serde::Deserialize)]
struct TemplateCase {
    #[serde(rename = "commandId")]
    command_id: String,
    params: HashMap<String, String>,
    #[serde(rename = "customTemplates")]
    custom_templates: Option<HashMap<String, String>>,
    resolved: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct InterpolationCase {
    template: String,
    params: HashMap<String, String>,
    result: String,
}

#[derive(Debug, serde::Deserialize)]
struct CapturedRequest {
    headers: HashMap<String, Option<String>>,
    body: serde_json::Value,
}

#[derive(Debug, serde::Deserialize)]
struct HttpCase {
    label: String,
    captured: CapturedRequest,
}

#[derive(Debug, serde::Deserialize)]
struct TtsParityData {
    #[serde(rename = "templateCases")]
    template_cases: Vec<TemplateCase>,
    #[serde(rename = "interpolationCases")]
    interpolation_cases: Vec<InterpolationCase>,
    #[serde(rename = "httpCases")]
    http_cases: Vec<HttpCase>,
}

async fn start_mock_server(
    status_code: u16,
    delay_ms: u64,
) -> (String, tokio::task::JoinHandle<(String, serde_json::Value)>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 8192];
        let n = socket.read(&mut buf).await.unwrap_or(0);
        let raw_req = String::from_utf8_lossy(&buf[..n]).to_string();

        if delay_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
        }

        let body_json = raw_req
            .split("\r\n\r\n")
            .nth(1)
            .and_then(|b| serde_json::from_str::<serde_json::Value>(b).ok())
            .unwrap_or_default();

        let response = format!(
            "HTTP/1.1 {} OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{{\"ok\":true}}",
            status_code
        );
        let _ = socket.write_all(response.as_bytes()).await;
        (raw_req, body_json)
    });
    (format!("http://{}", addr), handle)
}

#[test]
fn test_tts_plantillas_e_interpolacion_paridad_con_node() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let json_path = manifest_dir
        .join("tests")
        .join("data")
        .join("tts-casos.json");
    let content = fs::read_to_string(&json_path).expect("Failed reading tts-casos.json");
    let data: TtsParityData = serde_json::from_str(&content).expect("Invalid tts-casos.json");

    for c in &data.interpolation_cases {
        let res = interpolate_template(&c.template, &c.params);
        assert_eq!(
            res, c.result,
            "Interpolation mismatch for template='{}'",
            c.template
        );
    }

    for c in &data.template_cases {
        let res = resolve_feedback_message(&c.command_id, &c.params, c.custom_templates.as_ref());
        assert_eq!(
            res, c.resolved,
            "Template mismatch for command='{}'",
            c.command_id
        );
    }
}

#[tokio::test]
async fn test_tts_peticion_http_paridad_con_node() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let json_path = manifest_dir
        .join("tests")
        .join("data")
        .join("tts-casos.json");
    let content = fs::read_to_string(&json_path).expect("Failed reading tts-casos.json");
    let data: TtsParityData = serde_json::from_str(&content).expect("Invalid tts-casos.json");

    for c in &data.http_cases {
        let (api_url, server) = start_mock_server(200, 0).await;
        let payload = if c.label == "full_options" {
            let mut params = HashMap::new();
            params.insert("query".into(), "Bohemian Rhapsody".into());
            AvisoTts {
                guild_id: "guild-111".into(),
                channel_id: "channel-222".into(),
                user_id: Some("user-333".into()),
                command_id: "play".into(),
                params,
            }
        } else {
            AvisoTts {
                guild_id: "guild-999".into(),
                channel_id: "channel-888".into(),
                user_id: None,
                command_id: "pause".into(),
                params: HashMap::new(),
            }
        };

        let opts = if c.label == "full_options" {
            OpcionesTts {
                enabled: Some(true),
                api_url: Some(api_url),
                api_secret: Some("sec-secret-123".into()),
                voice: Some("es-MX-DaliaNeural".into()),
                rate: Some(1.25),
                pitch: Some(0.9),
                ..Default::default()
            }
        } else {
            let secret = c
                .captured
                .headers
                .get("authorization")
                .and_then(|a| a.as_ref())
                .and_then(|a| a.strip_prefix("Bearer "))
                .map(|s| s.to_string());
            OpcionesTts {
                enabled: Some(true),
                api_url: Some(api_url),
                api_secret: secret,
                ..Default::default()
            }
        };

        let ok = send_tts_feedback(&payload, Some(&opts)).await;
        assert!(ok);

        let (req_str, body_val) = server.await.unwrap();
        assert_eq!(body_val, c.captured.body);
        if let Some(expected_auth) = c
            .captured
            .headers
            .get("authorization")
            .and_then(|a| a.as_ref())
        {
            assert!(req_str
                .to_lowercase()
                .contains(&expected_auth.to_lowercase()));
        }
        if let Some(expected_uid) = c
            .captured
            .headers
            .get("x-user-discord-id")
            .and_then(|u| u.as_ref())
        {
            assert!(req_str
                .to_lowercase()
                .contains(&expected_uid.to_lowercase()));
        }
    }
}

#[tokio::test]
async fn test_tts_errores_y_apagado() {
    let (url_500, _) = start_mock_server(500, 0).await;
    let payload = AvisoTts {
        guild_id: "g1".into(),
        channel_id: "c1".into(),
        user_id: None,
        command_id: "pause".into(),
        params: HashMap::new(),
    };
    let opts_500 = OpcionesTts {
        api_url: Some(url_500),
        ..Default::default()
    };
    assert!(!send_tts_feedback(&payload, Some(&opts_500)).await);

    let (url_delay, _) = start_mock_server(200, 200).await;
    let opts_timeout = OpcionesTts {
        api_url: Some(url_delay),
        timeout_ms: Some(50),
        ..Default::default()
    };
    assert!(!send_tts_feedback(&payload, Some(&opts_timeout)).await);

    let opts_disabled = OpcionesTts {
        enabled: Some(false),
        api_url: Some("http://invalid".into()),
        ..Default::default()
    };
    assert!(!send_tts_feedback(&payload, Some(&opts_disabled)).await);
}
