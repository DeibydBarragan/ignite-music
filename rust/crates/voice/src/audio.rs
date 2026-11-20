//! Audio processing utilities for voice recognition (Spec S-07).
//!
//! Provides PCM downsampling (48kHz stereo to 16kHz mono), RMS calculation,
//! standard WAV container generation, and wake-word pre-roll audio slicing
//! with exact byte-by-byte parity with Node.js `audio-transcoder.js`.

/// Downsamples 48,000 Hz 16-bit stereo PCM to 16,000 Hz 16-bit mono PCM.
/// Ratio: 48000 / 16000 = 3:1.
///
/// Input is interleaved stereo samples `[L0, R0, L1, R1, ...]`.
/// Each stereo frame consists of 2 samples (L and R).
/// Each group of 3 stereo frames (6 samples) produces 1 mono sample taken
/// from the average of L and R of the first frame in that group.
pub fn downsample_48k_stereo_to_16k_mono(pcm: &[i16]) -> Vec<i16> {
    if pcm.len() < 6 {
        return Vec::new();
    }

    let frame_step = 6; // 3 frames * 2 channels
    let total_frames = pcm.len() / frame_step;
    let mut output = Vec::with_capacity(total_frames);

    for i in 0..total_frames {
        let offset = i * frame_step;
        let left = pcm[offset] as i32;
        let right = pcm[offset + 1] as i32;
        // JS Math.round((L + R) / 2) corresponds to div_euclid((L + R + 1), 2)
        let mono_sample = (left + right + 1).div_euclid(2) as i16;
        output.push(mono_sample);
    }

    output
}

/// Root-mean-square level of 16-bit mono PCM (0.0 = silence, 32768.0 = full scale).
/// Empty input returns 0.0.
pub fn pcm_rms(pcm: &[i16]) -> f64 {
    if pcm.is_empty() {
        return 0.0;
    }

    let mut sum_squares = 0.0f64;
    for &sample in pcm {
        let s = sample as f64;
        sum_squares += s * s;
    }

    (sum_squares / pcm.len() as f64).sqrt()
}

/// Wraps raw 16-bit PCM samples in a 44-byte standard RIFF/WAVE container.
pub fn pcm_to_wav(pcm: &[i16], sample_rate: u32, channels: u16) -> Vec<u8> {
    let data_size = (pcm.len() * 2) as u32;
    let bytes_per_sample = 2u16; // 16 bits / 8
    let block_align = channels * bytes_per_sample;
    let byte_rate = sample_rate * (block_align as u32);

    let mut wav = Vec::with_capacity(44 + data_size as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_size).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size (16 for PCM)
    wav.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat (1 = PCM)
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes()); // BitsPerSample: 16
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());

    for &sample in pcm {
        wav.extend_from_slice(&sample.to_le_bytes());
    }

    wav
}

/// Trims leading audio from a 16kHz mono 16-bit PCM slice up to `start_ms`,
/// preserving a `pre_roll_ms` buffer so the beginning of speech is not clipped.
///
/// Returns a borrowed view into `pcm` without copying.
pub fn slice_pcm(pcm: &[i16], start_ms: u32, pre_roll_ms: u32) -> &[i16] {
    if pcm.is_empty() || start_ms == 0 {
        return pcm;
    }

    let effective_start_ms = start_ms.saturating_sub(pre_roll_ms);
    // 16kHz * 2 bytes/sample / 1000 ms = 32 bytes per ms
    // In samples: 16 samples per ms
    let sample_offset = (effective_start_ms as usize) * 16;

    // Node: if (alignedOffset >= pcm16k.length - 12800) return pcm16k;
    // 12,800 bytes = 6,400 samples (400ms tail protection).
    if (sample_offset as i64) >= (pcm.len() as i64 - 6400) {
        return pcm;
    }

    &pcm[sample_offset..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rounding_parity_sums() {
        // Sum = -3: -2 + -1 = -3 -> Math.round(-1.5) = -1
        let pcm_neg3 = vec![-2, -1, 0, 0, 0, 0];
        let res_neg3 = downsample_48k_stereo_to_16k_mono(&pcm_neg3);
        assert_eq!(res_neg3, vec![-1]);

        // Sum = -1: -1 + 0 = -1 -> Math.round(-0.5) = 0
        let pcm_neg1 = vec![-1, 0, 0, 0, 0, 0];
        let res_neg1 = downsample_48k_stereo_to_16k_mono(&pcm_neg1);
        assert_eq!(res_neg1, vec![0]);

        // Sum = 1: 0 + 1 = 1 -> Math.round(0.5) = 1
        let pcm_pos1 = vec![0, 1, 0, 0, 0, 0];
        let res_pos1 = downsample_48k_stereo_to_16k_mono(&pcm_pos1);
        assert_eq!(res_pos1, vec![1]);

        // Sum = 3: 1 + 2 = 3 -> Math.round(1.5) = 2
        let pcm_pos3 = vec![1, 2, 0, 0, 0, 0];
        let res_pos3 = downsample_48k_stereo_to_16k_mono(&pcm_pos3);
        assert_eq!(res_pos3, vec![2]);
    }

    #[test]
    fn test_downsample_bounds_and_leftover() {
        assert_eq!(downsample_48k_stereo_to_16k_mono(&[]), Vec::<i16>::new());
        assert_eq!(
            downsample_48k_stereo_to_16k_mono(&[100, 200, 300]),
            Vec::<i16>::new()
        );

        // 11 samples: 1 frame of 6 used, 5 ignored
        let pcm = vec![1000, 3000, 500, 500, 200, 200, 99, 99, 99, 99, 99];
        let out = downsample_48k_stereo_to_16k_mono(&pcm);
        assert_eq!(out, vec![2000]);
    }

    #[test]
    fn test_rms_empty_and_silence() {
        assert_eq!(pcm_rms(&[]), 0.0);
        assert_eq!(pcm_rms(&[0, 0, 0, 0]), 0.0);
        assert_eq!(pcm_rms(&[100, 100, 100]), 100.0);
    }

    #[test]
    fn test_wav_header_exact_bytes() {
        let fake_pcm = vec![0i16; 800]; // 800 samples = 1600 bytes
        let wav = pcm_to_wav(&fake_pcm, 16000, 1);

        assert_eq!(wav.len(), 44 + 1600);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()), 36 + 1600);
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(u32::from_le_bytes(wav[16..20].try_into().unwrap()), 16);
        assert_eq!(u16::from_le_bytes(wav[20..22].try_into().unwrap()), 1); // PCM
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 1); // Channels
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16000); // Rate
        assert_eq!(u32::from_le_bytes(wav[28..32].try_into().unwrap()), 32000); // Byte rate
        assert_eq!(u16::from_le_bytes(wav[32..34].try_into().unwrap()), 2); // Block align
        assert_eq!(u16::from_le_bytes(wav[34..36].try_into().unwrap()), 16); // Bits per sample
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 1600);
    }

    #[test]
    fn test_slice_pcm_cases() {
        let pcm = vec![0i16; 32000]; // 2 seconds = 32000 samples

        // start_ms == 0 -> full
        assert_eq!(slice_pcm(&pcm, 0, 250).len(), 32000);

        // start_ms = 1000, pre_roll = 250 -> offset 750ms = 12000 samples -> len 20000
        let sliced = slice_pcm(&pcm, 1000, 250);
        assert_eq!(sliced.len(), 20000);

        // start_ms with pre-roll <= 0 -> starts at 0
        assert_eq!(slice_pcm(&pcm, 200, 250).len(), 32000);

        // Overslicing protection (wake word too close to tail)
        assert_eq!(slice_pcm(&pcm, 1900, 250).len(), 32000);

        // Audio shorter than 400ms (6400 samples) returns full audio
        let short_pcm = vec![0i16; 4000];
        assert_eq!(slice_pcm(&short_pcm, 100, 0).len(), 4000);

        // Empty returns empty
        let empty: [i16; 0] = [];
        assert_eq!(slice_pcm(&empty, 500, 250).len(), 0);
    }
}
