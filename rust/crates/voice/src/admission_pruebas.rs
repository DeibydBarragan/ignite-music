use super::*;

struct MockGate {
    passed: bool,
    start_ms: u32,
}

impl WakeGateEvaluator for MockGate {
    fn evaluate(&self, _pcm16k: &[i16], _wake_words: &str) -> GateEvaluation {
        GateEvaluation {
            passed: self.passed,
            reason: if self.passed {
                None
            } else {
                Some("closed".into())
            },
            heard: None,
            wake_word: None,
            start_ms: self.start_ms,
        }
    }
}

#[test]
fn test_admission_stages() {
    let controller = AdmissionController::new(AdmissionConfig {
        min_rms: 200.0,
        offline_gating: true,
        groq_cooldown_ms: 2000,
    });

    // 1. Low volume (silence)
    let silence = vec![0i16; 16000];
    let dec = controller.admit(1, 10, &silence, "ignite", None, 1000);
    assert!(matches!(dec, AdmissionDecision::LowVolume { .. }));

    // Loud audio (RMS ~1000)
    let loud = vec![1000i16; 16000];

    // 2. Gate closed
    let closed_gate = MockGate {
        passed: false,
        start_ms: 0,
    };
    let dec2 = controller.admit(1, 10, &loud, "ignite", Some(&closed_gate), 1000);
    assert!(matches!(dec2, AdmissionDecision::GateFilteredOut { .. }));

    // 3. Gate open -> Admitted
    let open_gate = MockGate {
        passed: true,
        start_ms: 500,
    };
    let dec3 = controller.admit(1, 10, &loud, "ignite", Some(&open_gate), 1000);
    assert!(matches!(dec3, AdmissionDecision::Admitted { .. }));

    // 4. Rate limited within 2000ms
    let dec4 = controller.admit(1, 10, &loud, "ignite", Some(&open_gate), 2500);
    assert_eq!(dec4, AdmissionDecision::RateLimited);

    // 5. Admitted after 2000ms passes
    let dec5 = controller.admit(1, 10, &loud, "ignite", Some(&open_gate), 3001);
    assert!(matches!(dec5, AdmissionDecision::Admitted { .. }));
}

#[test]
fn gate_no_disponible_no_admite_y_no_gasta_cooldown() {
    let controller = AdmissionController::new(AdmissionConfig {
        min_rms: 200.0,
        offline_gating: true,
        groq_cooldown_ms: 2000,
    });
    let loud = vec![1000i16; 16000];

    let sin_gate = controller.admit(1, 10, &loud, "ignite", None, 1000);
    assert_eq!(
        sin_gate,
        AdmissionDecision::GateFilteredOut {
            reason: Some("gate_unavailable".into()),
            heard: None,
        }
    );

    // Without a Groq request, the cooldown is still free at the same instant.
    let open_gate = MockGate {
        passed: true,
        start_ms: 0,
    };
    let dec = controller.admit(1, 10, &loud, "ignite", Some(&open_gate), 1000);
    assert!(matches!(dec, AdmissionDecision::Admitted { .. }));
}

#[test]
fn gating_desactivado_admite_sin_evaluador() {
    let controller = AdmissionController::new(AdmissionConfig {
        min_rms: 200.0,
        offline_gating: false,
        groq_cooldown_ms: 2000,
    });
    let loud = vec![1000i16; 16000];
    let dec = controller.admit(1, 10, &loud, "ignite", None, 1000);
    assert!(matches!(
        dec,
        AdmissionDecision::Admitted { gate: None, .. }
    ));
}
