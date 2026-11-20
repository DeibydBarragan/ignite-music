use medios::{describe_error, error_key, host_hint, YoutubeSearchErrorState, HINT_THROTTLE_MS};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct ErrorCase {
    nombre: String,
    codigo: Option<String>,
    mensaje: Option<String>,
    stderr: Option<String>,
    esperado: String,
}

#[test]
fn test_errores_paridad_con_node() {
    let json_bytes = include_bytes!("data/errores-casos.json");
    let cases: Vec<ErrorCase> = serde_json::from_slice(json_bytes).expect("parse json");
    assert!(!cases.is_empty(), "cases should not be empty");

    for c in cases {
        let actual = error_key(
            c.codigo.as_deref(),
            c.mensaje.as_deref(),
            c.stderr.as_deref(),
        );
        assert_eq!(
            actual, c.esperado,
            "Caso '{}' falló: esperado '{}', obtenido '{}'",
            c.nombre, c.esperado, actual
        );
    }
}

#[test]
fn test_describe_error_drm_protected_is_preserved() {
    let res = describe_error(
        "errors.drm_protected",
        Some("some msg"),
        None,
        Some(&YoutubeSearchErrorState {
            last_search_error: Some("errors.youtube_bot_detection".to_string()),
            last_search_error_at: 1000,
        }),
        0,
    );
    assert_eq!(res, "errors.drm_protected");
}

#[test]
fn test_describe_error_not_found_fallback_to_youtube_error() {
    let yt_state = YoutubeSearchErrorState {
        last_search_error: Some("errors.youtube_bot_detection".to_string()),
        last_search_error_at: 5000,
    };

    // Valid since window: should return youtube cause
    let res = describe_error(
        "musicplayer.no_results_found",
        Some("no items"),
        None,
        Some(&yt_state),
        4000,
    );
    assert_eq!(res, "errors.youtube_bot_detection");

    // Outdated search error (before since): should remain no_results_found
    let res_expired = describe_error(
        "musicplayer.no_results_found",
        Some("no items"),
        None,
        Some(&yt_state),
        6000,
    );
    assert_eq!(res_expired, "musicplayer.no_results_found");

    // Text mentions drm: should remain no_results_found
    let res_drm_text = describe_error(
        "musicplayer.no_results_found",
        Some("contains drm protected media"),
        None,
        Some(&yt_state),
        4000,
    );
    assert_eq!(res_drm_text, "musicplayer.no_results_found");

    // Cause is unknown: should remain no_results_found
    let yt_unknown = YoutubeSearchErrorState {
        last_search_error: Some("errors.unknown".to_string()),
        last_search_error_at: 5000,
    };
    let res_unknown_cause = describe_error(
        "musicplayer.no_results_found",
        Some("no items"),
        None,
        Some(&yt_unknown),
        4000,
    );
    assert_eq!(res_unknown_cause, "musicplayer.no_results_found");
}

#[test]
fn test_host_hint_throttling_and_keys() {
    let mut hints = HashMap::new();
    let t0 = 1_000_000;

    let bot_hint = host_hint("errors.youtube_bot_detection", t0, &mut hints);
    assert!(bot_hint.is_some());
    assert!(bot_hint
        .unwrap()
        .contains("Sign in to confirm you're not a bot"));

    // Immediate second call should be throttled
    let bot_hint_2 = host_hint("errors.youtube_bot_detection", t0 + 1000, &mut hints);
    assert!(bot_hint_2.is_none());

    // After 10 minutes (600_000 ms), hint is available again
    let bot_hint_3 = host_hint(
        "errors.youtube_bot_detection",
        t0 + HINT_THROTTLE_MS + 1,
        &mut hints,
    );
    assert!(bot_hint_3.is_some());

    // Other keys
    let age_hint = host_hint("errors.youtube_age_restricted", t0, &mut hints);
    assert!(age_hint.is_some());
    assert!(age_hint.unwrap().contains("Age-restricted videos"));

    let rate_hint = host_hint("errors.rate_limited", t0, &mut hints);
    assert!(rate_hint.is_some());
    assert!(rate_hint.unwrap().contains("rate limiting this server"));

    let unknown_hint = host_hint("errors.unknown", t0, &mut hints);
    assert!(unknown_hint.is_none());
}
