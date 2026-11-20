//! Integration tests for voice reception, segmentation and admission (Spec S-08).

use std::sync::Arc;
use voice::admission::{
    AdmissionConfig, AdmissionController, AdmissionDecision, GateEvaluation, WakeGateEvaluator,
};
use voice::receive::{ReceiverConfig, VoiceSegmenter, SAMPLES_PER_MS_48K_STEREO, TICK_MS};
use voice::speakers::{SpeakerFilter, SpeakerFilterConfig};

fn make_tick_pcm() -> Vec<i16> {
    vec![500i16; (TICK_MS as usize) * SAMPLES_PER_MS_48K_STEREO]
}

struct MockGate(bool);
impl WakeGateEvaluator for MockGate {
    fn evaluate(&self, _pcm16k: &[i16], _wake_words: &str) -> GateEvaluation {
        GateEvaluation {
            passed: self.0,
            reason: if self.0 { None } else { Some("closed".into()) },
            heard: None,
            wake_word: None,
            start_ms: 0,
        }
    }
}

#[tokio::test]
async fn test_segmentation_1000ms_voice_600ms_silence() {
    let filter = Arc::new(SpeakerFilter::new(SpeakerFilterConfig::default(), None));
    let mut seg = VoiceSegmenter::new(1, ReceiverConfig::default(), filter);
    seg.register_ssrc(10, Some(100));

    let tick_pcm = make_tick_pcm();
    let mut frags = Vec::new();

    // 50 ticks of voice = 1,000 ms
    for _ in 0..50 {
        frags.extend(seg.process_tick(&[(10, &tick_pcm)], &[]).await);
    }
    assert!(frags.is_empty());

    // 30 ticks of silence = 600 ms (silence cutoff is 500 ms)
    for _ in 0..30 {
        frags.extend(seg.process_tick(&[], &[10]).await);
    }

    assert_eq!(frags.len(), 1);
    assert_eq!(frags[0].user_id, 100);
    assert_eq!(frags[0].duration_ms, 1000);
}

#[tokio::test]
async fn test_segmentation_300ms_voice_discarded() {
    let filter = Arc::new(SpeakerFilter::new(SpeakerFilterConfig::default(), None));
    let mut seg = VoiceSegmenter::new(1, ReceiverConfig::default(), filter);
    seg.register_ssrc(10, Some(100));

    let tick_pcm = make_tick_pcm();
    let mut frags = Vec::new();

    // 15 ticks of voice = 300 ms (< 650 ms minimum)
    for _ in 0..15 {
        frags.extend(seg.process_tick(&[(10, &tick_pcm)], &[]).await);
    }
    // 30 ticks of silence
    for _ in 0..30 {
        frags.extend(seg.process_tick(&[], &[10]).await);
    }

    assert!(frags.is_empty(), "Audio < 650 ms must be discarded");
}

#[tokio::test]
async fn test_segmentation_13000ms_voice_cut_at_12000ms() {
    let filter = Arc::new(SpeakerFilter::new(SpeakerFilterConfig::default(), None));
    let mut seg = VoiceSegmenter::new(1, ReceiverConfig::default(), filter);
    seg.register_ssrc(10, Some(100));

    let tick_pcm = make_tick_pcm();
    let mut frags = Vec::new();

    // 650 ticks of voice = 13,000 ms
    for _ in 0..650 {
        frags.extend(seg.process_tick(&[(10, &tick_pcm)], &[]).await);
    }

    // At 12,000 ms, the first fragment was cut
    assert_eq!(frags.len(), 1);
    assert_eq!(frags[0].duration_ms, 12000);

    // Remaining 1,000 ms cut after silence
    for _ in 0..30 {
        frags.extend(seg.process_tick(&[], &[10]).await);
    }

    assert_eq!(frags.len(), 2);
    assert_eq!(frags[1].duration_ms, 1000);
}

#[tokio::test]
async fn test_segmentation_two_concurrent_users() {
    let filter = Arc::new(SpeakerFilter::new(SpeakerFilterConfig::default(), None));
    let mut seg = VoiceSegmenter::new(1, ReceiverConfig::default(), filter);
    seg.register_ssrc(10, Some(100));
    seg.register_ssrc(20, Some(200));

    let tick_pcm = make_tick_pcm();
    let mut frags = Vec::new();

    // Both speak for 50 ticks = 1,000 ms
    for _ in 0..50 {
        frags.extend(
            seg.process_tick(&[(10, &tick_pcm), (20, &tick_pcm)], &[])
                .await,
        );
    }

    // Silence for both
    for _ in 0..30 {
        frags.extend(seg.process_tick(&[], &[10, 20]).await);
    }

    assert_eq!(frags.len(), 2);
    let user_ids: Vec<u64> = frags.iter().map(|f| f.user_id).collect();
    assert!(user_ids.contains(&100));
    assert!(user_ids.contains(&200));
}

#[tokio::test]
async fn test_segmentation_bot_filtered_out() {
    let filter = Arc::new(SpeakerFilter::new(
        SpeakerFilterConfig {
            bot_id: Some(999),
            sound_bot_id: None,
            allow_bot_input: false,
        },
        None,
    ));
    filter.set_known_user(200, true); // User 200 is a bot

    let mut seg = VoiceSegmenter::new(1, ReceiverConfig::default(), filter);
    seg.register_ssrc(20, Some(200));

    let tick_pcm = make_tick_pcm();
    let mut frags = Vec::new();

    for _ in 0..50 {
        frags.extend(seg.process_tick(&[(20, &tick_pcm)], &[]).await);
    }
    for _ in 0..30 {
        frags.extend(seg.process_tick(&[], &[20]).await);
    }

    assert!(frags.is_empty(), "Bot audio must produce no fragments");
}

#[tokio::test]
async fn test_segmentation_client_disconnect() {
    let filter = Arc::new(SpeakerFilter::new(SpeakerFilterConfig::default(), None));
    let mut seg = VoiceSegmenter::new(1, ReceiverConfig::default(), filter);
    seg.register_ssrc(10, Some(100));

    let tick_pcm = make_tick_pcm();

    // 50 ticks = 1,000 ms
    for _ in 0..50 {
        let _ = seg.process_tick(&[(10, &tick_pcm)], &[]).await;
    }

    // Disconnect event fires
    let frag = seg.on_client_disconnect(100);
    assert!(frag.is_some());
    assert_eq!(frag.unwrap().duration_ms, 1000);
}

#[test]
fn test_admission_and_parity_decisions() {
    let ctrl = AdmissionController::new(AdmissionConfig {
        min_rms: 200.0,
        offline_gating: true,
        groq_cooldown_ms: 2000,
    });

    let loud_pcm = vec![1000i16; 16000]; // 1s at 16kHz, RMS = 1000
    let silence_pcm = vec![0i16; 16000];

    // Low RMS
    let res1 = ctrl.admit(1, 100, &silence_pcm, "ignite", None, 1000);
    assert!(matches!(res1, AdmissionDecision::LowVolume { .. }));

    // Closed gate
    let gate_closed = MockGate(false);
    let res2 = ctrl.admit(1, 100, &loud_pcm, "ignite", Some(&gate_closed), 1000);
    assert!(matches!(res2, AdmissionDecision::GateFilteredOut { .. }));

    // Open gate -> Admitted
    let gate_open = MockGate(true);
    let res3 = ctrl.admit(1, 100, &loud_pcm, "ignite", Some(&gate_open), 1000);
    assert!(matches!(res3, AdmissionDecision::Admitted { .. }));

    // Immediate second call (< 2000 ms) -> RateLimited
    let res4 = ctrl.admit(1, 100, &loud_pcm, "ignite", Some(&gate_open), 2000);
    assert_eq!(res4, AdmissionDecision::RateLimited);

    // Call after cooldown (3001 - 1000 > 2000) -> Admitted
    let res5 = ctrl.admit(1, 100, &loud_pcm, "ignite", Some(&gate_open), 3001);
    assert!(matches!(res5, AdmissionDecision::Admitted { .. }));
}
