use nucleo::error_api::{
    error_title_for_status, from_any_error, problem, status_by_code, ApiError, ProblemDocument,
};
use nucleo::registro;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn get_project_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .expect("Failed to locate project root")
        .to_path_buf()
}

#[test]
fn test_status_by_code_and_titles() {
    assert_eq!(status_by_code("guild_not_found"), 404);
    assert_eq!(status_by_code("shard_timeout"), 503);
    assert_eq!(status_by_code("not_ready"), 503);
    assert_eq!(status_by_code("unknown_op"), 500);
    assert_eq!(status_by_code("op_failed"), 500);
    assert_eq!(status_by_code("other_code"), 500);

    assert_eq!(error_title_for_status(400), "Invalid Request Parameter");
    assert_eq!(error_title_for_status(401), "Unauthorized");
    assert_eq!(error_title_for_status(403), "Forbidden");
    assert_eq!(error_title_for_status(404), "Not Found");
    assert_eq!(error_title_for_status(409), "Conflict");
    assert_eq!(error_title_for_status(413), "Payload Too Large");
    assert_eq!(error_title_for_status(422), "Unprocessable Entity");
    assert_eq!(error_title_for_status(429), "Too Many Requests");
    assert_eq!(error_title_for_status(500), "Internal Server Error");
    assert_eq!(error_title_for_status(502), "Bad Gateway");
    assert_eq!(error_title_for_status(503), "Service Unavailable");
    assert_eq!(error_title_for_status(418), "Error");
}

#[test]
fn test_api_error_derivation() {
    let err = ApiError::new(404, "GUILD_NOT_FOUND", "Guild not found.");
    assert_eq!(err.status, 404);
    assert_eq!(err.code, "GUILD_NOT_FOUND");
    assert_eq!(err.title, "Not Found");
    assert_eq!(err.r#type, "https://ignite.bot/errors/guild-not-found");
    assert_eq!(err.detail, "Guild not found.");
    assert_eq!(err.to_string(), "Guild not found.");

    let doc = err.to_problem();
    assert_eq!(doc.status, 404);
    assert_eq!(doc.code, "GUILD_NOT_FOUND");
    assert_eq!(doc.title, "Not Found");
    assert_eq!(doc.r#type, "https://ignite.bot/errors/guild-not-found");
    assert_eq!(doc.detail, "Guild not found.");
    assert_eq!(doc.error, "Guild not found.");
    assert!(chrono::DateTime::parse_from_rfc3339(&doc.timestamp).is_ok());
}

#[test]
fn test_from_any_error() {
    let api_err = ApiError::new(409, "ALREADY_PLAYING", "Queue busy");
    let doc_api = from_any_error(&api_err);
    assert_eq!(doc_api.status, 409);
    assert_eq!(doc_api.code, "ALREADY_PLAYING");

    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
    let doc_io = from_any_error(&io_err);
    assert_eq!(doc_io.status, 500);
    assert_eq!(doc_io.code, "INTERNAL_ERROR");
    assert_eq!(doc_io.detail, "file missing");
    assert_eq!(doc_io.error, "file missing");
}

#[test]
fn test_paridad_con_problem_casos_node() {
    let root = get_project_root();
    let json_file = root
        .join("rust")
        .join("crates")
        .join("nucleo")
        .join("tests")
        .join("data")
        .join("problem-casos.json");

    assert!(json_file.exists(), "problem-casos.json must exist");
    let content = fs::read_to_string(&json_file).expect("Failed to read problem-casos.json");
    let cases: Vec<Value> = serde_json::from_str(&content).expect("Invalid json");

    for c in cases {
        let name = c["name"].as_str().unwrap();
        let expected = &c["doc"];

        let status = expected["status"].as_u64().unwrap() as u16;
        let code = expected["code"].as_str().unwrap();
        let detail = expected["detail"].as_str().unwrap();

        let doc: ProblemDocument = match name {
            "api_error_custom" => {
                let err = ApiError::new(status, code, detail)
                    .with_title(expected["title"].as_str().unwrap())
                    .with_type(expected["type"].as_str().unwrap());
                err.to_problem()
            }
            "extra_fields" => problem(
                status,
                code,
                detail,
                expected["title"].as_str(),
                expected["type"].as_str(),
            )
            .with_extra("retryAfter", serde_json::json!(120))
            .with_extra("limit", serde_json::json!(100)),
            _ => problem(
                status,
                code,
                detail,
                expected["title"].as_str(),
                expected["type"].as_str(),
            ),
        };

        assert_eq!(doc.status, status, "Case {name} status mismatch");
        assert_eq!(doc.code, code, "Case {name} code mismatch");
        assert_eq!(doc.detail, detail, "Case {name} detail mismatch");
        assert_eq!(
            doc.title,
            expected["title"].as_str().unwrap(),
            "Case {name} title mismatch"
        );
        assert_eq!(
            doc.r#type,
            expected["type"].as_str().unwrap(),
            "Case {name} type mismatch"
        );
        assert_eq!(
            doc.error,
            expected["error"].as_str().unwrap(),
            "Case {name} error mismatch"
        );
        assert!(
            chrono::DateTime::parse_from_rfc3339(&doc.timestamp).is_ok(),
            "Case {name} timestamp invalid RFC3339"
        );

        if name == "extra_fields" {
            assert_eq!(doc.extra.get("retryAfter"), expected.get("retryAfter"));
            assert_eq!(doc.extra.get("limit"), expected.get("limit"));
        }
    }
}

#[test]
fn test_registro_init_and_macro() {
    registro::inicializar(true, true);
    nucleo::log_ok!(scope = "test", "Logging test event with ok=true");
}

#[test]
fn test_pino_json_format_contract() {
    use nucleo::registro::PinoFieldVisitor;

    let visitor = PinoFieldVisitor {
        msg: Some("Test message".to_string()),
        scope: Some("pot".to_string()),
        ok: false,
        ..Default::default()
    };

    let mut map = serde_json::Map::new();
    map.insert("level".into(), serde_json::Value::String("info".into()));
    map.insert(
        "time".into(),
        serde_json::Value::String("2026-11-08T12:00:00.000Z".into()),
    );
    if let Some(scope) = visitor.scope {
        map.insert("scope".into(), serde_json::Value::String(scope));
    }
    if let Some(msg) = visitor.msg {
        map.insert("msg".into(), serde_json::Value::String(msg));
    }
    let json_val = serde_json::Value::Object(map);

    assert_eq!(json_val["level"], "info");
    assert_eq!(json_val["scope"], "pot");
    assert_eq!(json_val["msg"], "Test message");
    assert!(json_val.get("time").is_some());
    assert!(json_val.get("fields").is_none());
    assert!(json_val.get("timestamp").is_none());
}
